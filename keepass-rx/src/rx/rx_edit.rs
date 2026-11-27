use std::collections::HashSet;
use std::fmt;

use gettextrs::gettext;
use keepass::db::{Database, EntryId, History, Times, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::constant_time_eq;
use crate::crypto::locking::SecretText;

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
    pub title: SecretText,
    pub username: SecretText,
    #[serde(default)]
    pub password: Option<SecretText>,
    pub url: SecretText,
    pub notes: SecretText,
    pub custom_fields: Vec<RxFieldEdit>,
    pub removed_fields: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RxFieldEdit {
    pub original_name: Option<String>,
    pub name: String,
    #[serde(default)]
    pub value: Option<SecretText>,
    pub protected: bool,
}

impl RxFieldEdit {
    fn stored_name(&self) -> &str {
        self.original_name.as_deref().unwrap_or(&self.name)
    }
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
        Some(value) => constant_time_eq(value.get(), text),
        None => text.is_empty(),
    }
}

impl RxEntryEdit {
    fn id(&self) -> EntryId {
        EntryId::from_uuid(self.uuid)
    }

    pub fn fill_unchanged(&mut self, draft: &RxEntryEdit) {
        if self.uuid != draft.uuid {
            return;
        }
        if self.password.is_none() {
            self.password = draft.password.clone();
        }
        for field in self.custom_fields.iter_mut().filter(|f| f.value.is_none()) {
            field.value = draft
                .custom_fields
                .iter()
                .find(|d| d.original_name.is_some() && d.original_name == field.original_name)
                .and_then(|d| d.value.clone());
        }
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
            if field.value.is_none() && !existing.contains(field.stored_name()) {
                return Err(EditError::Invalid(format!(
                    "field {:?} has no value",
                    field.name
                )));
            }
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
            let protected = password.is_some_and(Value::is_protected);
            let replacement = match (&self.password, password) {
                (Some(text), current) if !holds(current, text) || !protected => {
                    Some(value_of(text, true))
                }
                (None, Some(current)) if !protected => Some(value_of(current.get(), true)),
                _ => None,
            };
            if let Some(value) = replacement {
                tracked.fields.insert(PASSWORD.into(), value);
            }

            for name in &self.removed_fields {
                tracked.fields.remove(name);
            }
            let changed: Vec<(&RxFieldEdit, Value<String>)> = self
                .custom_fields
                .iter()
                .filter(|field| {
                    let current = tracked.fields.get(&field.name);
                    field.original_name.as_deref() != Some(field.name.as_str())
                        || field
                            .value
                            .as_ref()
                            .is_some_and(|text| !holds(current, text))
                        || current.is_some_and(Value::is_protected) != field.protected
                })
                .filter_map(|field| {
                    let text = match &field.value {
                        Some(text) => text.as_str(),
                        None => tracked.fields.get(field.stored_name())?.get().as_str(),
                    };
                    Some((field, value_of(text, field.protected)))
                })
                .collect();
            for (field, _) in &changed {
                if let Some(original) = &field.original_name {
                    tracked.fields.remove(original);
                }
            }
            for (field, value) in changed {
                tracked.fields.insert(field.name.clone(), value);
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
        if let Some(password) = &self.password {
            check(PASSWORD, password)?;
        }
        let password = entry.fields.get(PASSWORD);
        if (self.password.is_some() || password.is_some())
            && !password.is_some_and(Value::is_protected)
        {
            return Err(EditError::Mismatch("password is not protected".into()));
        }

        for field in &self.custom_fields {
            match &field.value {
                Some(value) => check(&field.name, value)?,
                None if !entry.fields.contains_key(&field.name) => {
                    return Err(EditError::Mismatch(format!(
                        "field {:?} is missing",
                        field.name
                    )));
                }
                None => {}
            }
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
#[path = "rx_edit_tests.rs"]
mod tests;
