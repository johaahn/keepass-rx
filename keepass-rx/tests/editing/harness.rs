use keepass::{Database, DatabaseKey, db::EntryId};
use keepassrx::crypto::locking::SecretText;
use keepassrx::rx::{
    NoHooks, RxDatabase, RxEdit, RxEntryEdit, RxFieldEdit, RxSourceFile, SaveError, SaveHooks,
    SaveOutcome, SaveStep,
    open::{open_database, read_database},
    save_database,
};
use keyring::set_default_credential_builder;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Once;
use uuid::Uuid;

pub const PASSWORD: &str = "demopass";
pub const V4_40: &str = "test_db_kdbx4_with_password_argon2.kdbx";
pub const V3: &str = "test_db_with_password.kdbx";
pub const FULL: &str = "test_db_kpxc_full.kdbx";
pub const BACKUP_DIR: &str = "backups";

pub const ALL_STEPS: [SaveStep; 10] = [
    SaveStep::ReadOriginal,
    SaveStep::Parse,
    SaveStep::ApplyEdit,
    SaveStep::Serialize,
    SaveStep::VerifyNew,
    SaveStep::RecheckOriginal,
    SaveStep::WriteBackup,
    SaveStep::WriteDatabase,
    SaveStep::SwapDatabase,
    SaveStep::VerifyFinal,
];

pub fn step_index(step: SaveStep) -> usize {
    ALL_STEPS
        .iter()
        .position(|s| *s == step)
        .expect("known step")
}

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn key() -> DatabaseKey {
    DatabaseKey::new().with_password(PASSWORD)
}

pub fn s(value: &str) -> SecretText {
    SecretText::new(value.to_string())
}

pub struct FailAt(pub SaveStep, pub io::ErrorKind);

impl SaveHooks for FailAt {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step == self.0 {
            Err(io::Error::new(self.1, "injected"))
        } else {
            Ok(())
        }
    }
}

pub struct MutateAt {
    pub step: SaveStep,
    pub path: PathBuf,
    pub replacement: Vec<u8>,
}

impl MutateAt {
    pub fn new(step: SaveStep, vault: &TestVault) -> Self {
        MutateAt {
            step,
            path: vault.path(),
            replacement: fs::read(fixture_path(V3)).expect("read replacement fixture"),
        }
    }
}

impl SaveHooks for MutateAt {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step == self.step {
            fs::write(&self.path, &self.replacement)?;
        }
        Ok(())
    }
}

pub struct TestVault {
    dir: tempfile::TempDir,
    name: String,
}

impl TestVault {
    pub fn from_fixture(name: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::copy(fixture_path(name), dir.path().join(name)).expect("copy fixture");
        TestVault {
            dir,
            name: name.to_string(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.path().join(&self.name)
    }

    pub fn backup_path(&self) -> PathBuf {
        self.dir.path().join(BACKUP_DIR).join("db.bak")
    }

    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).expect("read database")
    }

    pub fn backup_bytes(&self) -> Vec<u8> {
        fs::read(self.backup_path()).expect("read backup")
    }

    pub fn open(&self) -> (RxDatabase, RxSourceFile) {
        static MOCK: Once = Once::new();
        MOCK.call_once(|| {
            set_default_credential_builder(keyring::mock::default_credential_builder());
        });
        open_database(&self.path(), key()).expect("open database")
    }

    pub fn source(&self) -> RxSourceFile {
        read_database(&self.path(), key()).expect("read database").1
    }

    pub fn save(
        &self,
        src: &RxSourceFile,
        edit: Option<&RxEdit>,
        hooks: &dyn SaveHooks,
    ) -> Result<SaveOutcome, SaveError> {
        save_database(
            src,
            key(),
            &self.backup_path(),
            |db| match edit {
                Some(edit) => edit.apply(db).map_err(Into::into),
                None => Ok(()),
            },
            |db| match edit {
                Some(edit) => edit.verify(db).map_err(Into::into),
                None => Ok(()),
            },
            hooks,
        )
    }

    pub fn save_now(&self, edit: &RxEdit) -> SaveOutcome {
        self.save(&self.source(), Some(edit), &NoHooks)
            .expect("save")
    }

    pub fn listing(&self) -> BTreeSet<String> {
        fs::read_dir(self.dir.path())
            .expect("read dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect()
    }

    pub fn only_database(&self) -> BTreeSet<String> {
        BTreeSet::from([self.name.clone()])
    }

    pub fn database_and_backups(&self) -> BTreeSet<String> {
        BTreeSet::from([self.name.clone(), BACKUP_DIR.to_string()])
    }

    pub fn reopen_raw(&self) -> Database {
        Database::parse(&self.bytes(), key()).expect("parse database")
    }

    pub fn xml(&self) -> String {
        let xml =
            Database::get_xml(&mut fs::File::open(self.path()).expect("open file"), key())
                .expect("get xml");
        String::from_utf8(xml).expect("utf8 xml")
    }
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "Title" | "UserName" | "Password" | "URL" | "Notes" | "otp"
    ) || matches!(
        name,
        "KeePassXC-Browser Settings" | "_LAST_MODIFIED" | "_etm_template_uuid"
    ) || ["AndroidApp", "KP2A_URL", "KPEX_PASSKEY_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

pub fn entry_uuid(db: &Database, title: &str) -> Uuid {
    db.iter_all_entries()
        .find(|e| e.get("Title") == Some(title))
        .unwrap_or_else(|| panic!("entry {title} missing"))
        .id()
        .uuid()
}

pub fn unchanged_edit(db: &Database, uuid: Uuid) -> RxEntryEdit {
    let entry = db.entry(EntryId::from_uuid(uuid)).expect("entry");
    let text = |key: &str| s(entry.get(key).unwrap_or_default());
    let custom_fields = entry
        .fields
        .iter()
        .filter(|(name, _)| !is_reserved(name))
        .map(|(name, value)| RxFieldEdit {
            original_name: Some(name.clone()),
            name: name.clone(),
            value: s(value.get()),
            protected: value.is_protected(),
        })
        .collect();
    RxEntryEdit {
        uuid,
        title: text("Title"),
        username: text("UserName"),
        password: text("Password"),
        url: text("URL"),
        notes: text("Notes"),
        custom_fields,
        removed_fields: vec![],
    }
}

pub fn password_edit(db: &Database, uuid: Uuid, password: &str) -> RxEdit {
    let mut edit = unchanged_edit(db, uuid);
    edit.password = s(password);
    RxEdit::UpdateEntry(edit)
}

pub fn password_of(db: &Database, uuid: Uuid) -> String {
    db.entry(EntryId::from_uuid(uuid))
        .expect("entry")
        .get("Password")
        .expect("password")
        .to_string()
}

pub fn history_passwords(db: &Database, uuid: Uuid) -> Vec<String> {
    db.entry(EntryId::from_uuid(uuid))
        .expect("entry")
        .history
        .as_ref()
        .map(|h| {
            h.get_entries()
                .iter()
                .map(|e| e.get("Password").unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}
