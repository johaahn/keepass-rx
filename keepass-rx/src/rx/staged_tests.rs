use super::*;
use crate::rx::save::{NoHooks, RxDbFormat, SaveStep, sha256_of};
use keepass::Database;
use keepass::config::DatabaseVersion;
use keepass::db::EntryId;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use uuid::Uuid;

const PASSWORD: &str = "demopass";
const FIXTURE: &str = "test_db_kdbx4_with_password_argon2.kdbx";
const NEW_PASSWORD: &str = "staged-secret-4711";
const KEY_FILE: &[u8] = b"staged key file bytes";
const NEW_NOTES: &str = "staged-notes-0815";

struct Vault {
    dir: tempfile::TempDir,
}

impl Vault {
    fn new() -> Self {
        libsodium_rs::ensure_init().expect("libsodium");
        let dir = tempfile::tempdir().expect("tempdir");
        let from = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(FIXTURE);
        fs::copy(from, dir.path().join(FIXTURE)).expect("copy fixture");
        Vault { dir }
    }
    fn path(&self) -> PathBuf {
        self.dir.path().join(FIXTURE)
    }
    fn staged(&self) -> PathBuf {
        self.dir.path().join("staged").join("db.kdbx.edit")
    }
    fn staging(&self) -> StagingKey {
        let (_, src) = self.open();
        open_staging_key(&self.staged(), self.key(), &src.staging_config).expect("staging key")
    }
    fn backup(&self) -> PathBuf {
        self.dir.path().join("backups").join("db.bak")
    }
    fn key(&self) -> DatabaseKey {
        DatabaseKey::new().with_password(PASSWORD)
    }
    fn open(&self) -> (Database, RxSourceFile) {
        let bytes = fs::read(self.path()).expect("read db");
        let db = Database::parse(&bytes, self.key()).expect("parse");
        let src = RxSourceFile {
            path: self.path(),
            format: RxDbFormat::from_version(&db.config.version),
            sha256: sha256_of(&bytes),
            staging_config: db.config.clone(),
        };
        (db, src)
    }
    fn staged_edit(&self) -> (StagedEdit, RxSourceFile) {
        let (db, src) = self.open();
        let entry = db.iter_all_entries().next().expect("an entry");
        let text = |key| Zeroizing::new(entry.get(key).unwrap_or_default().to_string());
        let edit = RxEntryEdit {
            uuid: entry.id().uuid(),
            title: text("Title"),
            username: text("UserName"),
            password: Zeroizing::new(NEW_PASSWORD.to_string()),
            url: text("URL"),
            notes: Zeroizing::new(NEW_NOTES.to_string()),
            custom_fields: vec![],
            removed_fields: vec![],
        };
        let base = snapshot(&HashMap::from([(
            "password".to_string(),
            EditValue::Text(entry.get_password().unwrap_or_default().to_string()),
        )]));
        let staged = StagedEdit {
            edit,
            base_sha256: src.sha256,
            base,
            draft: false,
        };
        (staged, src)
    }
    fn save(
        &self,
        src: &RxSourceFile,
        edit: RxEntryEdit,
        hooks: &dyn SaveHooks,
    ) -> Result<SaveOutcome, SaveError> {
        save_and_unstage(
            src,
            self.key(),
            &self.backup(),
            &self.staged(),
            &RxEdit::UpdateEntry(edit),
            hooks,
        )
    }
}

struct FailAt(SaveStep);

impl SaveHooks for FailAt {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step == self.0 {
            Err(io::Error::other("injected"))
        } else {
            Ok(())
        }
    }
}

const ALL_STEPS: [SaveStep; 10] = [
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

#[test]
fn staged_edit_round_trips() {
    let vault = Vault::new();
    let (staged, _) = vault.staged_edit();
    write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");

    let read = read_staged(&vault.staged(), &vault.staging())
        .expect("read")
        .expect("present");
    assert_eq!(read.edit.uuid, staged.edit.uuid);
    assert_eq!(read.edit.password.as_str(), NEW_PASSWORD);
    assert_eq!(read.edit.notes.as_str(), NEW_NOTES);
    assert_eq!(read.edit.title, staged.edit.title);
    assert_eq!(read.base_sha256, staged.base_sha256);
    assert_eq!(read.base, staged.base);
}

#[test]
fn missing_staged_file_reads_as_none() {
    let vault = Vault::new();
    assert!(
        read_staged(&vault.staged(), &vault.staging())
            .expect("read")
            .is_none()
    );
    remove_staged(&vault.staged()).expect("remove missing");
}

#[test]
fn wrong_password_fails_and_keeps_the_file() {
    let vault = Vault::new();
    let (staged, src) = vault.staged_edit();
    write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");

    let err = open_staging_key(
        &vault.staged(),
        DatabaseKey::new().with_password("not-the-password"),
        &src.staging_config,
    )
    .expect_err("error");
    assert!(matches!(err, StageError::Decrypt), "{err:?}");
    assert!(vault.staged().exists());
}

#[test]
fn staged_file_hides_entry_values_and_is_private() {
    let vault = Vault::new();
    let (staged, _) = vault.staged_edit();
    write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");

    let bytes = fs::read(vault.staged()).expect("read");
    let text = String::from_utf8_lossy(&bytes);
    let uuid = staged.edit.uuid.to_string();
    let secrets = [
        NEW_PASSWORD,
        NEW_NOTES,
        staged.edit.title.as_str(),
        uuid.as_str(),
        PASSWORD,
    ];
    for secret in secrets.into_iter().filter(|s| !s.is_empty()) {
        assert!(!text.contains(secret), "{secret} is in the staged file");
    }
    for path in [vault.staged(), staging_key_path(&vault.staged())] {
        let mode = fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn entry_change_is_detected_against_the_base() {
    let vault = Vault::new();
    let (staged, src) = vault.staged_edit();
    let other_sha = [0u8; 32];
    let changed = snapshot(&HashMap::from([(
        "password".to_string(),
        EditValue::Text("changed elsewhere".into()),
    )]));
    assert!(!staged.entry_changed(&src.sha256, None));
    assert!(!staged.entry_changed(&other_sha, Some(&staged.base)));
    assert!(staged.entry_changed(&other_sha, Some(&changed)));
    assert!(staged.entry_changed(&other_sha, None));
}

#[test]
fn staging_key_file_uses_the_database_config() {
    let vault = Vault::new();
    vault.staging();

    let bytes = fs::read(staging_key_path(&vault.staged())).expect("read");
    let db = Database::parse(&bytes, vault.key()).expect("parse staged");
    let (source_db, _) = vault.open();
    let mut expected = source_db.config;
    expected.version = DatabaseVersion::KDB4(1);
    assert_eq!(db.config, expected);
    assert_eq!(db.iter_all_entries().count(), 1);
}

#[test]
fn staged_file_needs_the_key_file() {
    let vault = Vault::new();
    let (staged, src) = vault.staged_edit();
    let keyed = || {
        vault
            .key()
            .with_keyfile(&mut { KEY_FILE })
            .expect("key file")
    };
    let staging = |key| open_staging_key(&vault.staged(), key, &src.staging_config);
    write_staged(&vault.staged(), &staging(keyed()).expect("key"), &staged).expect("write");

    let read = read_staged(&vault.staged(), &staging(keyed()).expect("key")).expect("read");
    assert!(read.is_some());
    for key in [
        vault.key(),
        vault
            .key()
            .with_keyfile(&mut "other key file".as_bytes())
            .expect("key file"),
    ] {
        let err = staging(key).expect_err("error");
        assert!(matches!(err, StageError::Decrypt), "{err:?}");
    }
    assert!(vault.staged().exists());
}

#[test]
fn staging_key_is_reused_and_needed() {
    let vault = Vault::new();
    let (mut staged, _) = vault.staged_edit();
    staged.draft = true;
    let staging = vault.staging();
    assert_eq!(vault.staging(), staging);
    write_staged(&vault.staged(), &staging, &staged).expect("write");

    let read = read_staged(&vault.staged(), &staging)
        .expect("read")
        .expect("present");
    assert!(read.draft);
    let err = read_staged(&vault.staged(), &StagingKey::generate())
        .err()
        .expect("error");
    assert!(matches!(err, StageError::Decrypt), "{err:?}");

    fs::remove_file(staging_key_path(&vault.staged())).expect("remove key");
    let (_, src) = vault.open();
    let err = open_staging_key(&vault.staged(), vault.key(), &src.staging_config)
        .expect_err("error");
    assert!(matches!(err, StageError::MissingKey), "{err:?}");
}

#[test]
fn renewed_staging_key_replaces_the_old_one() {
    let vault = Vault::new();
    let old = vault.staging();
    let (_, src) = vault.open();
    let next = write_staging_key(
        &next_staging_key_path(&vault.staged()),
        vault.key(),
        &src.staging_config,
    )
    .expect("write next key");
    assert_eq!(vault.staging(), old);

    replace_staging_key(&vault.staged()).expect("replace");
    assert!(!next_staging_key_path(&vault.staged()).exists());
    let opened = vault.staging();
    assert_eq!(opened, next);
    assert_ne!(opened, old);
}

#[test]
fn renewal_moves_through_pending_to_renewed() {
    let old = StagingKey::generate();
    let next = StagingKey::generate();

    let mut staging = Staging::Loaded(old.clone());
    assert!(staging.start_renewal());
    assert!(!staging.start_renewal());
    staging.hold(next.clone());
    assert!(matches!(staging.key(), Some(key) if *key == old));
    staging.commit();
    assert!(matches!(&staging, Staging::Renewed(key) if *key == next));
    assert!(!staging.start_renewal());

    let mut staging = Staging::Loaded(old.clone());
    staging.start_renewal();
    staging.abandon();
    assert!(matches!(&staging, Staging::Renewed(key) if *key == old));

    let mut staging = Staging::None;
    staging.hold(next);
    staging.commit();
    assert!(matches!(staging, Staging::None));
}

#[test]
fn changes_name_the_edited_fields() {
    let text = |value: &str| EditValue::Text(value.to_string());
    let field = |name: &str, value: &str| {
        EditValue::Map(HashMap::from([
            ("name".to_string(), text(name)),
            ("value".to_string(), text(value)),
            ("protected".to_string(), EditValue::Flag(false)),
        ]))
    };
    let base = snapshot(&HashMap::from([
        ("title".to_string(), text("Mail")),
        ("username".to_string(), text("me")),
        ("password".to_string(), text("old")),
        ("url".to_string(), text("https://old")),
        ("notes".to_string(), text("")),
        (
            "customFields".to_string(),
            EditValue::List(vec![
                field("PIN", "1234"),
                field("Kept", "same"),
                field("Gone", "x"),
            ]),
        ),
    ]));
    let custom = |original: Option<&str>, name: &str, value: &str| RxFieldEdit {
        original_name: original.map(str::to_string),
        name: name.to_string(),
        value: Zeroizing::new(value.to_string()),
        protected: false,
    };
    let staged = StagedEdit {
        edit: RxEntryEdit {
            uuid: Uuid::nil(),
            title: Zeroizing::new("Mail".into()),
            username: Zeroizing::new("me".into()),
            password: Zeroizing::new("new".into()),
            url: Zeroizing::new(String::new()),
            notes: Zeroizing::new("note".into()),
            custom_fields: vec![
                custom(Some("PIN"), "Code", "1234"),
                custom(Some("Kept"), "Kept", "same"),
                custom(None, "Added", "v"),
            ],
            removed_fields: vec!["Gone".to_string()],
        },
        base_sha256: [0; 32],
        base,
        draft: false,
    };

    let change = |name: &str, custom: bool, kind: ChangeKind| StagedChange {
        name: name.to_string(),
        custom,
        kind,
    };
    assert_eq!(
        staged.changes(),
        vec![
            change("password", false, ChangeKind::Modified),
            change("url", false, ChangeKind::Deleted),
            change("notes", false, ChangeKind::Added),
            change("Code", true, ChangeKind::Modified),
            change("Added", true, ChangeKind::Added),
            change("Gone", true, ChangeKind::Deleted),
        ]
    );
}

#[test]
fn staged_file_survives_a_failure_at_every_step() {
    for step in ALL_STEPS {
        let vault = Vault::new();
        let (staged, src) = vault.staged_edit();
        write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");

        let err = vault.save(&src, staged.edit, &FailAt(step)).err();
        assert!(err.is_some(), "{step:?}");
        let kept = read_staged(&vault.staged(), &vault.staging())
            .expect("read")
            .expect("still staged");
        assert_eq!(kept.edit.password.as_str(), NEW_PASSWORD, "{step:?}");
    }
}

#[test]
fn staged_file_survives_a_conflict() {
    let vault = Vault::new();
    let (staged, src) = vault.staged_edit();
    write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");
    let mut changed = fs::read(vault.path()).unwrap();
    changed.push(0);
    fs::write(vault.path(), changed).unwrap();

    let err = vault
        .save(&src, staged.edit, &NoHooks)
        .err()
        .expect("error");
    assert!(matches!(err, SaveError::Conflict), "{err:?}");
    assert!(vault.staged().exists());
}

#[test]
fn staged_file_is_removed_after_a_successful_save() {
    let vault = Vault::new();
    let (staged, src) = vault.staged_edit();
    let uuid: Uuid = staged.edit.uuid;
    write_staged(&vault.staged(), &vault.staging(), &staged).expect("write");

    vault.save(&src, staged.edit, &NoHooks).expect("save");
    assert!(!vault.staged().exists());
    let (db, _) = vault.open();
    let entry = db.entry(EntryId::from_uuid(uuid)).expect("entry");
    assert_eq!(entry.get_password(), Some(NEW_PASSWORD));
}
