use std::collections::HashSet;
use std::fmt;

use gettextrs::gettext;
use keepass::db::{Database, EntryId, History, Times, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::rx_entry::should_hide_field;
use super::save::SaveError;

const TITLE: &str = "Title";
const USERNAME: &str = "UserName";
const PASSWORD: &str = "Password";
const URL: &str = "URL";
const NOTES: &str = "Notes";
const OTP: &str = "otp";

const STANDARD_KEYS: [&str; 6] = [TITLE, USERNAME, PASSWORD, URL, NOTES, OTP];

#[derive(Clone)]
pub enum RxEdit {
    UpdateEntry(RxEntryEdit),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RxEntryEdit {
    pub uuid: Uuid,
    pub title: Zeroizing<String>,
    pub username: Zeroizing<String>,
    pub password: Zeroizing<String>,
    pub url: Zeroizing<String>,
    pub notes: Zeroizing<String>,
    pub custom_fields: Vec<RxFieldEdit>,
    pub removed_fields: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RxFieldEdit {
    pub original_name: Option<String>,
    pub name: String,
    pub value: Zeroizing<String>,
    pub protected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditProblem {
    pub index: Option<usize>,
    pub message: String,
}

#[derive(Debug)]
pub enum EditError {
    EntryNotFound(Uuid),
    Invalid(String),
    Mismatch(String),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::EntryNotFound(uuid) => write!(f, "Entry {uuid} not found"),
            EditError::Invalid(reason) => write!(f, "Invalid edit: {reason}"),
            EditError::Mismatch(reason) => write!(f, "Edit verification failed: {reason}"),
        }
    }
}

impl std::error::Error for EditError {}

impl From<EditError> for SaveError {
    fn from(err: EditError) -> Self {
        SaveError::Edit(err.to_string())
    }
}

impl RxEdit {
    pub fn apply(&self, db: &mut Database) -> Result<(), EditError> {
        match self {
            RxEdit::UpdateEntry(edit) => edit.apply(db),
        }
    }

    pub fn verify(&self, db: &Database) -> Result<(), EditError> {
        match self {
            RxEdit::UpdateEntry(edit) => edit.verify(db),
        }
    }
}

fn is_reserved(name: &str) -> bool {
    STANDARD_KEYS.contains(&name) || should_hide_field(name)
}

fn value_of(text: &str, protected: bool) -> Value<String> {
    if protected {
        Value::protected(text.to_string())
    } else {
        Value::unprotected(text.to_string())
    }
}

fn holds(current: Option<&Value<String>>, text: &str) -> bool {
    match current {
        Some(value) => value.get() == text,
        None => text.is_empty(),
    }
}

impl RxEntryEdit {
    fn id(&self) -> EntryId {
        EntryId::from_uuid(self.uuid)
    }

    fn validate(&self) -> Result<(), EditError> {
        match self.problems().into_iter().next() {
            Some(problem) => Err(EditError::Invalid(problem.message)),
            None => Ok(()),
        }
    }

    pub fn problems(&self) -> Vec<EditProblem> {
        let mut problems = Vec::new();
        let mut seen = HashSet::new();
        for (index, field) in self.custom_fields.iter().enumerate() {
            let mut problem = |message: String| {
                problems.push(EditProblem {
                    index: Some(index),
                    message,
                })
            };
            if field.name.is_empty() {
                problem(gettext("Field name is empty"));
            } else if is_reserved(&field.name) {
                problem(gettext("Field name {} is reserved").replace("{}", &field.name));
            } else if !seen.insert(field.name.as_str()) {
                problem(gettext("Duplicate field name {}").replace("{}", &field.name));
            }
            if let Some(original) = &field.original_name
                && is_reserved(original)
            {
                problem(gettext("Field {} cannot be edited").replace("{}", original));
            }
        }

        let mut removed = HashSet::new();
        for name in &self.removed_fields {
            let message = if is_reserved(name) {
                gettext("Field {} cannot be removed")
            } else if !removed.insert(name.as_str()) {
                gettext("Field {} is removed twice")
            } else if seen.contains(name.as_str()) {
                gettext("Field {} is both removed and set")
            } else {
                continue;
            };
            problems.push(EditProblem {
                index: None,
                message: message.replace("{}", name),
            });
        }
        problems
    }

    fn apply(&self, db: &mut Database) -> Result<(), EditError> {
        self.validate()?;

        let history_max = db
            .meta
            .history_max_items
            .and_then(|max| usize::try_from(max).ok());

        let existing: HashSet<String> = db
            .entry(self.id())
            .ok_or(EditError::EntryNotFound(self.uuid))?
            .fields
            .keys()
            .cloned()
            .collect();

        let mut vacated: HashSet<&str> =
            self.removed_fields.iter().map(String::as_str).collect();
        vacated.extend(
            self.custom_fields
                .iter()
                .filter_map(|f| f.original_name.as_deref()),
        );
        for field in &self.custom_fields {
            let same = field.original_name.as_deref() == Some(field.name.as_str());
            if !same
                && existing.contains(&field.name)
                && !vacated.contains(field.name.as_str())
            {
                return Err(EditError::Invalid(format!(
                    "field {:?} already exists",
                    field.name
                )));
            }
        }

        let mut entry = db
            .entry_mut(self.id())
            .ok_or(EditError::EntryNotFound(self.uuid))?;

        entry.edit_tracking(|tracked| {
            for (key, text) in [
                (TITLE, &self.title),
                (USERNAME, &self.username),
                (URL, &self.url),
                (NOTES, &self.notes),
            ] {
                if !holds(tracked.fields.get(key), text) {
                    let protected = tracked.fields.get(key).is_some_and(Value::is_protected);
                    tracked.fields.insert(key.into(), value_of(text, protected));
                }
            }
            let password = tracked.fields.get(PASSWORD);
            if !holds(password, &self.password) || !password.is_some_and(Value::is_protected) {
                tracked
                    .fields
                    .insert(PASSWORD.into(), value_of(&self.password, true));
            }

            for name in &self.removed_fields {
                tracked.fields.remove(name);
            }
            let changed: Vec<&RxFieldEdit> = self
                .custom_fields
                .iter()
                .filter(|field| {
                    let current = tracked.fields.get(&field.name);
                    field.original_name.as_deref() != Some(field.name.as_str())
                        || !holds(current, &field.value)
                        || current.is_some_and(Value::is_protected) != field.protected
                })
                .collect();
            for field in &changed {
                if let Some(original) = &field.original_name {
                    tracked.fields.remove(original);
                }
            }
            for field in &changed {
                tracked
                    .fields
                    .insert(field.name.clone(), value_of(&field.value, field.protected));
            }
            tracked.times.last_modification = Some(Times::now());
        });

        let history = entry.history.take().unwrap_or_default();
        let mut items = history.get_entries().clone();
        if !items.is_empty() {
            items.rotate_left(1);
        }
        if let Some(max) = history_max
            && items.len() > max
        {
            items.drain(..items.len() - max);
        }
        let mut rebuilt = History::default();
        for item in items.into_iter().rev() {
            rebuilt.add_entry(item);
        }
        entry.history = Some(rebuilt);

        Ok(())
    }

    fn verify(&self, db: &Database) -> Result<(), EditError> {
        let entry = db
            .entry(self.id())
            .ok_or(EditError::EntryNotFound(self.uuid))?;

        let check = |key: &str, expected: &str| -> Result<(), EditError> {
            if holds(entry.fields.get(key), expected) {
                Ok(())
            } else {
                Err(EditError::Mismatch(format!("field {key:?} differs")))
            }
        };
        check(TITLE, &self.title)?;
        check(USERNAME, &self.username)?;
        check(URL, &self.url)?;
        check(NOTES, &self.notes)?;
        check(PASSWORD, &self.password)?;
        if !entry.fields.get(PASSWORD).is_some_and(Value::is_protected) {
            return Err(EditError::Mismatch("password is not protected".into()));
        }

        for field in &self.custom_fields {
            check(&field.name, &field.value)?;
            let protected = entry
                .fields
                .get(&field.name)
                .is_some_and(Value::is_protected);
            if protected != field.protected {
                return Err(EditError::Mismatch(format!(
                    "field {:?} has the wrong protection",
                    field.name
                )));
            }
            if let Some(original) = &field.original_name
                && original != &field.name
                && entry.fields.contains_key(original)
            {
                return Err(EditError::Mismatch(format!(
                    "field {original:?} was not renamed"
                )));
            }
        }
        for name in &self.removed_fields {
            if entry.fields.contains_key(name) {
                return Err(EditError::Mismatch(format!(
                    "field {name:?} was not removed"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_PASS: &str = "old-pass";

    fn s(value: &str) -> Zeroizing<String> {
        Zeroizing::new(value.to_string())
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
            password: s(OLD_PASS),
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
            value: s("red"),
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
        edit.password = s("new-pass");
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
            value: s("1234"),
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
    fn removes_and_adds_fields() {
        let (mut db, uuid) = test_db();
        let mut edit = base_edit(uuid);
        edit.removed_fields.push("Color".into());
        edit.custom_fields.push(RxFieldEdit {
            original_name: None,
            name: "Extra".into(),
            value: s("x"),
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
            edit.password = s("new-pass");
            for name in names {
                edit.custom_fields.push(RxFieldEdit {
                    original_name: None,
                    name: name.into(),
                    value: s("v"),
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
        edit.password = s("new-pass");
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
        entry_edit.password = s("new-pass");
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
}
