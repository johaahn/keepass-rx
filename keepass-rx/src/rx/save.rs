use atomic_write_file::AtomicWriteFile;
use keepass::{
    Database, DatabaseKey,
    config::{DatabaseConfig, DatabaseVersion},
    error::{DatabaseOpenError, DatabaseSaveError},
};
use rustix::{
    fs::{CWD, RenameFlags, renameat_with},
    io::Errno,
};
use sha2::{Digest, Sha256};

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::TempPath;
use zeroize::Zeroizing;

use super::staged::StageError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RxDbFormat {
    Kdb,
    Kdbx3,
    Kdbx4,
}

impl RxDbFormat {
    pub fn from_version(version: &DatabaseVersion) -> Self {
        match version {
            DatabaseVersion::KDB(_) => RxDbFormat::Kdb,
            DatabaseVersion::KDB2(_) | DatabaseVersion::KDB3(_) => RxDbFormat::Kdbx3,
            DatabaseVersion::KDB4(_) => RxDbFormat::Kdbx4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RxSourceFile {
    pub path: PathBuf,
    pub format: RxDbFormat,
    pub sha256: [u8; 32],
    pub staging_config: DatabaseConfig,
}

pub fn sha256_of(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("The database file changed on disk since it was opened")]
    Conflict,
    #[error("This database format cannot be saved")]
    UnsupportedFormat,
    #[error("Verification failed: {0}")]
    Verify(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Could not read database: {0}")]
    Open(#[from] DatabaseOpenError),
    #[error("Could not write database: {0}")]
    Save(#[from] DatabaseSaveError),
    #[error("{0}")]
    Edit(String),
    #[error("{0}")]
    Stage(#[from] StageError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SaveStep {
    ReadOriginal,
    Parse,
    ApplyEdit,
    Serialize,
    VerifyNew,
    RecheckOriginal,
    WriteBackup,
    WriteDatabase,
    SwapDatabase,
    VerifyFinal,
}

pub trait SaveHooks {
    fn before(&self, step: SaveStep) -> io::Result<()>;
}

pub struct NoHooks;

impl SaveHooks for NoHooks {
    fn before(&self, _step: SaveStep) -> io::Result<()> {
        Ok(())
    }
}

pub struct SaveOutcome {
    pub db: Database,
    pub sha256: [u8; 32],
    pub conflict_copy: Option<PathBuf>,
}

fn read_checked(path: &Path, expected: &[u8; 32]) -> Result<Zeroizing<Vec<u8>>, SaveError> {
    let bytes = Zeroizing::new(fs::read(path)?);
    if sha256_of(&bytes) != *expected {
        return Err(SaveError::Conflict);
    }
    Ok(bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(bytes)?;
    file.commit()
}

/// Whether the kernel, filesystem or device lacks `RENAME_EXCHANGE`.
fn is_exchange_unsupported(err: Errno) -> bool {
    matches!(err, Errno::INVAL | Errno::NOSYS | Errno::OPNOTSUPP)
}

fn exchange(temp: &Path, database: &Path) -> io::Result<bool> {
    match renameat_with(CWD, temp, CWD, database, RenameFlags::EXCHANGE) {
        Ok(()) => Ok(true),
        Err(err) if is_exchange_unsupported(err) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

fn conflict_path(database: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let stem = database.file_stem().unwrap_or_default().to_string_lossy();
    let name = match database.extension() {
        Some(ext) => format!("{stem}.conflict-{stamp}.{}", ext.to_string_lossy()),
        None => format!("{stem}.conflict-{stamp}"),
    };
    database.with_file_name(name)
}

fn keep_if_changed(old: TempPath, src: &RxSourceFile) -> io::Result<Option<PathBuf>> {
    if fs::read(&old).is_ok_and(|bytes| sha256_of(&bytes) == src.sha256) {
        return Ok(None);
    }
    let kept = old.keep()?;
    let conflict = conflict_path(&src.path);
    Ok(Some(
        fs::rename(&kept, &conflict).map_or(kept, |()| conflict),
    ))
}

fn write_database(
    src: &RxSourceFile,
    bytes: &[u8],
    hooks: &dyn SaveHooks,
) -> Result<Option<PathBuf>, SaveError> {
    let dir = src.path.parent().unwrap_or(Path::new("."));
    let mut file = tempfile::Builder::new()
        .prefix(".keepassrx-")
        .tempfile_in(dir)?;
    file.write_all(bytes)?;
    file.as_file()
        .set_permissions(fs::metadata(&src.path)?.permissions())?;
    file.as_file().sync_all()?;
    let temp = file.into_temp_path();

    hooks.before(SaveStep::SwapDatabase)?;
    let conflict = if exchange(&temp, &src.path)? {
        keep_if_changed(temp, src)?
    } else {
        temp.persist(&src.path).map_err(|err| err.error)?;
        None
    };
    fs::File::open(dir)?.sync_all()?;
    Ok(conflict)
}

const BACKUPS_KEPT: usize = 3;

fn backup_generation(backup_path: &Path, generation: usize) -> PathBuf {
    if generation == 0 {
        return backup_path.to_path_buf();
    }
    let mut name = backup_path.as_os_str().to_owned();
    name.push(format!(".{generation}"));
    PathBuf::from(name)
}

fn rotate_backups(backup_path: &Path) -> io::Result<()> {
    for generation in (0..BACKUPS_KEPT - 1).rev() {
        let from = backup_generation(backup_path, generation);
        match fs::rename(&from, backup_generation(backup_path, generation + 1)) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            result => result?,
        }
    }
    Ok(())
}

pub fn save_database(
    src: &RxSourceFile,
    key: DatabaseKey,
    backup_path: &Path,
    edit: impl FnOnce(&mut Database) -> Result<(), SaveError>,
    verify: impl Fn(&Database) -> Result<(), SaveError>,
    hooks: &dyn SaveHooks,
) -> Result<SaveOutcome, SaveError> {
    if src.format != RxDbFormat::Kdbx4 {
        return Err(SaveError::UnsupportedFormat);
    }

    hooks.before(SaveStep::ReadOriginal)?;
    let original = read_checked(&src.path, &src.sha256)?;

    hooks.before(SaveStep::Parse)?;
    let mut db = Database::parse(&original, key.clone())?;

    hooks.before(SaveStep::ApplyEdit)?;
    if db.config.version == DatabaseVersion::KDB4(0) {
        db.config.version = DatabaseVersion::KDB4(1);
    }
    edit(&mut db)?;

    hooks.before(SaveStep::Serialize)?;
    let mut new_bytes = Zeroizing::new(Vec::new());
    db.save(&mut *new_bytes, key.clone())?;
    drop(db);

    hooks.before(SaveStep::VerifyNew)?;
    let reopened = Database::parse(&new_bytes, key)?;
    verify(&reopened)?;
    let new_hash = sha256_of(&new_bytes);

    hooks.before(SaveStep::RecheckOriginal)?;
    let original = {
        drop(original);
        read_checked(&src.path, &src.sha256)?
    };

    hooks.before(SaveStep::WriteBackup)?;
    if let Some(dir) = backup_path.parent() {
        fs::create_dir_all(dir)?;
    }
    rotate_backups(backup_path)?;
    write_atomic(backup_path, &original)?;

    hooks.before(SaveStep::WriteDatabase)?;
    let conflict_copy = write_database(src, &new_bytes, hooks)?;

    hooks.before(SaveStep::VerifyFinal)?;
    if sha256_of(&fs::read(&src.path)?) != new_hash {
        return Err(SaveError::Verify("file changed after saving".into()));
    }

    Ok(SaveOutcome {
        db: reopened,
        sha256: new_hash,
        conflict_copy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeSet;

    const PASSWORD: &str = "demopass";
    const V4_40: &str = "test_db_kdbx4_with_password_argon2.kdbx";
    const V3: &str = "test_db_with_password.kdbx";

    struct Vault {
        dir: tempfile::TempDir,
        name: &'static str,
    }

    impl Vault {
        fn new(name: &'static str) -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let from = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            fs::copy(from, dir.path().join(name)).expect("copy fixture");
            Vault { dir, name }
        }
        fn path(&self) -> PathBuf {
            self.dir.path().join(self.name)
        }
        fn backup(&self) -> PathBuf {
            self.dir.path().join("backups").join("db.bak")
        }
        fn bytes(&self) -> Vec<u8> {
            fs::read(self.path()).expect("read db")
        }
        fn backup_names(&self) -> BTreeSet<String> {
            fs::read_dir(self.dir.path().join("backups"))
                .expect("read backups")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .collect()
        }
        fn key(&self) -> DatabaseKey {
            DatabaseKey::new().with_password(PASSWORD)
        }
        fn source(&self) -> RxSourceFile {
            let bytes = self.bytes();
            let db = Database::parse(&bytes, self.key()).expect("parse");
            RxSourceFile {
                path: self.path(),
                format: RxDbFormat::from_version(&db.config.version),
                sha256: sha256_of(&bytes),
                staging_config: db.config.clone(),
            }
        }
        fn save(
            &self,
            src: &RxSourceFile,
            hooks: &dyn SaveHooks,
        ) -> Result<SaveOutcome, SaveError> {
            save_database(
                src,
                self.key(),
                &self.backup(),
                |_| Ok(()),
                |_| Ok(()),
                hooks,
            )
        }
        fn listing(&self) -> BTreeSet<String> {
            fs::read_dir(self.dir.path())
                .expect("read dir")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .collect()
        }
    }

    fn entry_ids(db: &Database) -> BTreeSet<(String, String)> {
        let mut out = BTreeSet::new();
        for e in db.iter_all_entries() {
            out.insert((
                e.id().uuid().to_string(),
                e.get("Title").unwrap_or_default().to_string(),
            ));
        }
        out
    }

    struct FailAt(SaveStep, io::ErrorKind);

    impl SaveHooks for FailAt {
        fn before(&self, step: SaveStep) -> io::Result<()> {
            if step == self.0 {
                Err(io::Error::new(self.1, "injected"))
            } else {
                Ok(())
            }
        }
    }

    struct Recorder(Cell<usize>);

    impl SaveHooks for Recorder {
        fn before(&self, _step: SaveStep) -> io::Result<()> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }

    #[test]
    fn noop_save_upgrades_40_to_41() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        assert_eq!(src.format, RxDbFormat::Kdbx4);
        let before = vault.bytes();
        let original = Database::parse(&before, vault.key()).unwrap();
        assert_eq!(original.config.version, DatabaseVersion::KDB4(0));

        let recorder = Recorder(Cell::new(0));
        let outcome = vault.save(&src, &recorder).expect("save");
        assert_eq!(recorder.0.get(), 10);
        assert_eq!(outcome.conflict_copy, None);

        let after = vault.bytes();
        assert_eq!(outcome.sha256, sha256_of(&after));
        let saved = Database::parse(&after, vault.key()).unwrap();
        assert_eq!(saved.config.version, DatabaseVersion::KDB4(1));
        assert_eq!(entry_ids(&saved), entry_ids(&original));
        assert_eq!(entry_ids(&outcome.db), entry_ids(&original));
        assert_eq!(fs::read(vault.backup()).unwrap(), before);
        assert_eq!(
            vault.listing(),
            BTreeSet::from([V4_40.to_string(), "backups".to_string()])
        );
    }

    #[test]
    fn only_unsupported_errnos_fall_back() {
        for errno in [Errno::INVAL, Errno::NOSYS, Errno::OPNOTSUPP] {
            assert!(is_exchange_unsupported(errno), "{errno:?}");
        }
        for errno in [Errno::ACCESS, Errno::NOENT, Errno::IO, Errno::XDEV] {
            assert!(!is_exchange_unsupported(errno), "{errno:?}");
        }
    }

    #[test]
    fn saved_file_opens_in_keepassxc() {
        use std::process::{Command, Stdio};
        let cli = Path::new("/usr/bin/keepassxc-cli");
        if !cli.exists() {
            eprintln!("keepassxc-cli not installed, skipping");
            return;
        }
        let vault = Vault::new(V4_40);
        vault.save(&vault.source(), &NoHooks).expect("save");
        let mut child = Command::new(cli)
            .arg("db-info")
            .arg(vault.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{PASSWORD}\n").as_bytes())
            .unwrap();
        let out = child.wait_with_output().expect("wait");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{text} {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(text.contains("Name: Demo-KDBX4"), "{text}");
        assert!(text.contains("Number of entries: 2"), "{text}");
    }

    #[test]
    fn changed_file_is_a_conflict() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        let mut changed = vault.bytes();
        changed.push(0);
        fs::write(vault.path(), &changed).unwrap();

        let err = vault.save(&src, &NoHooks).err().expect("error");
        assert!(matches!(err, SaveError::Conflict), "{err:?}");
        assert_eq!(vault.bytes(), changed);
        assert_eq!(vault.listing(), BTreeSet::from([V4_40.to_string()]));
    }

    #[test]
    fn change_before_the_write_is_a_conflict() {
        struct Mutate(PathBuf);
        impl SaveHooks for Mutate {
            fn before(&self, step: SaveStep) -> io::Result<()> {
                if step == SaveStep::RecheckOriginal {
                    let mut bytes = fs::read(&self.0)?;
                    bytes.push(1);
                    fs::write(&self.0, bytes)?;
                }
                Ok(())
            }
        }
        let vault = Vault::new(V4_40);
        let src = vault.source();
        let err = vault
            .save(&src, &Mutate(vault.path()))
            .err()
            .expect("error");
        assert!(matches!(err, SaveError::Conflict), "{err:?}");
        assert_eq!(vault.listing(), BTreeSet::from([V4_40.to_string()]));
    }

    #[test]
    fn kdbx3_is_unsupported() {
        let vault = Vault::new(V3);
        let src = vault.source();
        assert_eq!(src.format, RxDbFormat::Kdbx3);
        let before = vault.bytes();
        let err = vault.save(&src, &NoHooks).err().expect("error");
        assert!(matches!(err, SaveError::UnsupportedFormat), "{err:?}");
        assert_eq!(vault.bytes(), before);
        assert_eq!(vault.listing(), BTreeSet::from([V3.to_string()]));
    }

    #[test]
    fn failure_before_the_write_leaves_the_original() {
        let steps = [
            SaveStep::ReadOriginal,
            SaveStep::Parse,
            SaveStep::ApplyEdit,
            SaveStep::Serialize,
            SaveStep::VerifyNew,
            SaveStep::RecheckOriginal,
            SaveStep::WriteBackup,
        ];
        for step in steps {
            let vault = Vault::new(V4_40);
            let src = vault.source();
            let before = vault.bytes();
            let err = vault
                .save(&src, &FailAt(step, io::ErrorKind::Other))
                .err()
                .expect("error");
            assert!(matches!(err, SaveError::Io(_)), "{step:?}: {err:?}");
            assert_eq!(vault.bytes(), before, "{step:?}");
            assert_eq!(
                vault.listing(),
                BTreeSet::from([V4_40.to_string()]),
                "{step:?}"
            );
        }
    }

    #[test]
    fn full_disk_at_the_database_write_leaves_the_original() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        let before = vault.bytes();
        let err = vault
            .save(
                &src,
                &FailAt(SaveStep::WriteDatabase, io::ErrorKind::StorageFull),
            )
            .err()
            .expect("error");
        match err {
            SaveError::Io(e) => assert_eq!(e.kind(), io::ErrorKind::StorageFull),
            other => panic!("{other:?}"),
        }
        assert_eq!(vault.bytes(), before);
        assert_eq!(fs::read(vault.backup()).unwrap(), before);
        assert_eq!(
            vault.listing(),
            BTreeSet::from([V4_40.to_string(), "backups".to_string()])
        );
        assert_eq!(vault.backup_names(), BTreeSet::from(["db.bak".to_string()]));
    }

    #[test]
    fn four_saves_keep_the_last_three_backups() {
        let vault = Vault::new(V4_40);
        let mut versions = vec![vault.bytes()];
        for _ in 0..4 {
            let src = vault.source();
            vault.save(&src, &NoHooks).expect("save");
            versions.push(vault.bytes());
        }
        let backup = vault.backup();
        let read = |generation| fs::read(backup_generation(&backup, generation)).unwrap();
        assert_eq!(read(0), versions[3]);
        assert_eq!(read(1), versions[2]);
        assert_eq!(read(2), versions[1]);
        assert_eq!(
            vault.backup_names(),
            BTreeSet::from(["db.bak", "db.bak.1", "db.bak.2"].map(String::from))
        );
    }

    #[test]
    fn a_conflict_leaves_the_backups_unchanged() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        vault.save(&src, &NoHooks).expect("save");
        let src = vault.source();
        let names = vault.backup_names();
        let backup = fs::read(vault.backup()).unwrap();
        fs::write(vault.path(), b"changed").unwrap();
        let err = vault.save(&src, &NoHooks).err().expect("error");
        assert!(matches!(err, SaveError::Conflict), "{err:?}");
        assert_eq!(vault.backup_names(), names);
        assert_eq!(fs::read(vault.backup()).unwrap(), backup);
    }

    #[test]
    fn failure_after_the_write_keeps_the_new_file() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        let before = vault.bytes();
        let err = vault
            .save(&src, &FailAt(SaveStep::VerifyFinal, io::ErrorKind::Other))
            .err()
            .expect("error");
        assert!(matches!(err, SaveError::Io(_)));
        assert_ne!(vault.bytes(), before);
        Database::parse(&vault.bytes(), vault.key()).expect("new file opens");
        assert_eq!(fs::read(vault.backup()).unwrap(), before);
    }

    #[test]
    fn edit_and_verify_errors_abort_the_save() {
        let vault = Vault::new(V4_40);
        let src = vault.source();
        let before = vault.bytes();
        let err = save_database(
            &src,
            vault.key(),
            &vault.backup(),
            |_| Err(SaveError::Edit("nope".into())),
            |_| Ok(()),
            &NoHooks,
        )
        .err()
        .expect("error");
        assert!(matches!(err, SaveError::Edit(_)));
        let err = save_database(
            &src,
            vault.key(),
            &vault.backup(),
            |_| Ok(()),
            |_| Err(SaveError::Verify("bad".into())),
            &NoHooks,
        )
        .err()
        .expect("error");
        assert!(matches!(err, SaveError::Verify(_)));
        assert_eq!(vault.bytes(), before);
        assert_eq!(vault.listing(), BTreeSet::from([V4_40.to_string()]));
    }
}
