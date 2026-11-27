use super::*;

const OLD_PASS: &str = "old-pass";

fn s(value: &str) -> SecretText {
    SecretText::new(value.to_string())
}

fn test_db() -> (Database, Uuid) {
    let mut db = Database::new();
    let uuid = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.edit(|e| {
            e.set_unprotected("Title", "v0");
            e.set_unprotected("UserName", "bob");
            e.set_protected("Password", OLD_PASS);
            e.set_unprotected("URL", "https://example.com");
            e.set_unprotected("Notes", "notes");
            e.set_protected("PIN", "1234");
            e.set_unprotected("Color", "red");
            e.set_unprotected("_etm_template_uuid", "abc");
        });
        for title in ["v1", "v2"] {
            entry.edit_tracking(|t| t.set_unprotected("Title", title));
        }
        entry.id().uuid()
    };
    (db, uuid)
}

fn base_edit(uuid: Uuid) -> RxEntryEdit {
    RxEntryEdit {
        uuid,
        title: s("v2"),
        username: s("bob"),
        password: Some(s(OLD_PASS)),
        url: s("https://example.com"),
        notes: s("notes"),
        custom_fields: vec![],
        removed_fields: vec![],
    }
}

fn history_titles(db: &Database, uuid: Uuid) -> Vec<String> {
    db.entry(EntryId::from_uuid(uuid))
        .unwrap()
        .history
        .as_ref()
        .unwrap()
        .get_entries()
        .iter()
        .map(|e| e.fields["Title"].get().clone())
        .collect()
}

#[test]
fn edits_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<RxEdit>();
    assert_send::<EditError>();
}

#[test]
fn unchanged_fields_are_not_written() {
    let (mut db, uuid) = test_db();
    db.entry_mut(EntryId::from_uuid(uuid)).unwrap().edit(|e| {
        e.fields.remove(URL);
        e.fields.remove(NOTES);
        e.set_unprotected("Color", "red");
    });
    let mut edit = base_edit(uuid);
    edit.url = s("");
    edit.notes = s("");
    edit.title = s("v3");
    edit.custom_fields.push(RxFieldEdit {
        original_name: Some("Color".into()),
        name: "Color".into(),
        value: Some(s("red")),
        protected: false,
    });
    let edit = RxEdit::UpdateEntry(edit);

    edit.apply(&mut db).unwrap();
    edit.verify(&db).unwrap();

    let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
    assert!(!entry.fields.contains_key(URL));
    assert!(!entry.fields.contains_key(NOTES));
    assert_eq!(entry.fields["Title"].get(), "v3");
    assert_eq!(entry.fields["Color"].get(), "red");
}

#[test]
fn password_change_appends_old_version_to_history() {
    let (mut db, uuid) = test_db();
    let before = history_titles(&db, uuid);
    assert_eq!(before.len(), 2);
    let mut edit = base_edit(uuid);
    edit.password = Some(s("new-pass"));
    let edit = RxEdit::UpdateEntry(edit);

    edit.apply(&mut db).unwrap();
    edit.verify(&db).unwrap();

    let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
    assert_eq!(entry.get_password(), Some("new-pass"));
    let items = entry.history.as_ref().unwrap().get_entries();
    assert_eq!(items.len(), before.len() + 1);
    assert_eq!(items.last().unwrap().get_password(), Some(OLD_PASS));
    assert_eq!(history_titles(&db, uuid)[..2], before[..]);
}

#[test]
fn rename_keeps_value_and_protection() {
    let (mut db, uuid) = test_db();
    let mut edit = base_edit(uuid);
    edit.custom_fields.push(RxFieldEdit {
        original_name: Some("PIN".into()),
        name: "Code".into(),
        value: Some(s("1234")),
        protected: true,
    });
    let edit = RxEdit::UpdateEntry(edit);

    edit.apply(&mut db).unwrap();
    edit.verify(&db).unwrap();

    let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
    assert!(!entry.fields.contains_key("PIN"));
    assert_eq!(entry.fields["Code"].get(), "1234");
    assert!(entry.fields["Code"].is_protected());
    assert_eq!(entry.fields["Color"].get(), "red");
    assert_eq!(entry.fields["_etm_template_uuid"].get(), "abc");
}

#[test]
fn absent_values_keep_the_stored_ones() {
    let (mut db, uuid) = test_db();
    let mut edit = base_edit(uuid);
    edit.password = None;
    edit.title = s("v3");
    edit.custom_fields.push(RxFieldEdit {
        original_name: Some("PIN".into()),
        name: "Code".into(),
        value: None,
        protected: true,
    });
    let edit = RxEdit::UpdateEntry(edit);

    edit.apply(&mut db).unwrap();
    edit.verify(&db).unwrap();

    let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
    assert_eq!(entry.get_password(), Some(OLD_PASS));
    assert!(entry.fields[PASSWORD].is_protected());
    assert!(!entry.fields.contains_key("PIN"));
    assert_eq!(entry.fields["Code"].get(), "1234");
    assert!(entry.fields["Code"].is_protected());
}

#[test]
fn absent_value_of_unknown_field_is_invalid() {
    let (mut db, uuid) = test_db();
    let mut edit = base_edit(uuid);
    edit.custom_fields.push(RxFieldEdit {
        original_name: None,
        name: "Extra".into(),
        value: None,
        protected: false,
    });
    let edit = RxEdit::UpdateEntry(edit);
    assert!(matches!(edit.apply(&mut db), Err(EditError::Invalid(_))));
}

#[test]
fn fill_unchanged_takes_the_draft_values() {
    let uuid = Uuid::new_v4();
    let field = |value: Option<&str>| RxFieldEdit {
        original_name: Some("PIN".into()),
        name: "Code".into(),
        value: value.map(s),
        protected: true,
    };
    let mut draft = base_edit(uuid);
    draft.password = Some(s("draft-pass"));
    draft.custom_fields.push(field(Some("9999")));
    let mut edit = base_edit(uuid);
    edit.password = None;
    edit.custom_fields.push(field(None));

    edit.fill_unchanged(&draft);

    assert_eq!(edit.password.as_deref(), Some("draft-pass"));
    assert_eq!(edit.custom_fields[0].value.as_deref(), Some("9999"));
}

#[test]
fn removes_and_adds_fields() {
    let (mut db, uuid) = test_db();
    let mut edit = base_edit(uuid);
    edit.removed_fields.push("Color".into());
    edit.custom_fields.push(RxFieldEdit {
        original_name: None,
        name: "Extra".into(),
        value: Some(s("x")),
        protected: false,
    });
    let edit = RxEdit::UpdateEntry(edit);
    edit.apply(&mut db).unwrap();
    edit.verify(&db).unwrap();
    let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
    assert!(!entry.fields.contains_key("Color"));
    assert_eq!(entry.fields["Extra"].get(), "x");
}

#[test]
fn unknown_entry_is_not_found() {
    let (mut db, _) = test_db();
    let edit = RxEdit::UpdateEntry(base_edit(Uuid::new_v4()));
    assert!(matches!(
        edit.apply(&mut db),
        Err(EditError::EntryNotFound(_))
    ));
}

#[test]
fn reserved_and_duplicate_names_are_invalid_and_change_nothing() {
    for names in [
        vec!["_etm_template_uuid", "x"],
        vec!["Title"],
        vec!["Password"],
        vec!["Dup", "Dup"],
        vec!["Color"],
        vec![""],
    ] {
        let (mut db, uuid) = test_db();
        let before = db.entry(EntryId::from_uuid(uuid)).unwrap().fields.clone();
        let mut edit = base_edit(uuid);
        edit.password = Some(s("new-pass"));
        for name in names {
            edit.custom_fields.push(RxFieldEdit {
                original_name: None,
                name: name.into(),
                value: Some(s("v")),
                protected: false,
            });
        }
        let edit = RxEdit::UpdateEntry(edit);
        assert!(matches!(edit.apply(&mut db), Err(EditError::Invalid(_))));
        let entry = db.entry(EntryId::from_uuid(uuid)).unwrap();
        assert_eq!(entry.fields, before);
        assert_eq!(entry.history.as_ref().unwrap().get_entries().len(), 2);
    }
}

#[test]
fn verify_reports_mismatch() {
    let (mut db, uuid) = test_db();
    let mut edit = base_edit(uuid);
    edit.password = Some(s("new-pass"));
    let edit = RxEdit::UpdateEntry(edit);
    assert!(matches!(edit.verify(&db), Err(EditError::Mismatch(_))));
    edit.apply(&mut db).unwrap();
    assert!(edit.verify(&db).is_ok());
}

#[test]
fn saved_password_change_survives_reopen() {
    use crate::rx::save::{NoHooks, RxDbFormat, RxSourceFile, save_database, sha256_of};
    use keepass::DatabaseKey;

    let name = "test_db_kpxc_full.kdbx";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
        &path,
    )
    .unwrap();
    let key = || DatabaseKey::new().with_password("demopass");
    let bytes = std::fs::read(&path).unwrap();
    let db = Database::parse(&bytes, key()).unwrap();
    let src = RxSourceFile {
        path: path.clone(),
        format: RxDbFormat::from_version(&db.config.version),
        sha256: sha256_of(&bytes),
        staging_config: db.config.clone(),
    };
    let entry = db
        .iter_all_entries()
        .find(|e| e.get_title() == Some("Full Entry"))
        .unwrap();
    let uuid = entry.id().uuid();
    let old_password = entry.get_password().unwrap().to_string();
    let mut entry_edit = base_edit(uuid);
    entry_edit.title = s("Full Entry");
    entry_edit.username = s(entry.get_username().unwrap_or_default());
    entry_edit.url = s(entry.get(URL).unwrap_or_default());
    entry_edit.notes = s(entry.get(NOTES).unwrap_or_default());
    entry_edit.password = Some(s("new-pass"));
    drop(db);
    let edit = RxEdit::UpdateEntry(entry_edit);

    save_database(
        &src,
        key(),
        &dir.path().join("backup.kdbx"),
        |db| edit.apply(db).map_err(Into::into),
        |db| edit.verify(db).map_err(Into::into),
        &NoHooks,
    )
    .unwrap();

    let reopened = Database::parse(&std::fs::read(&path).unwrap(), key()).unwrap();
    let entry = reopened.entry(EntryId::from_uuid(uuid)).unwrap();
    assert_eq!(entry.get_password(), Some("new-pass"));
    let items = entry.history.as_ref().unwrap().get_entries();
    assert_eq!(items.len(), 4);
    assert_eq!(
        items.last().unwrap().get_password(),
        Some(old_password.as_str())
    );
}
