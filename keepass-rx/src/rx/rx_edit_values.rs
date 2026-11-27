use std::collections::HashMap;

use uuid::Uuid;

use crate::crypto::locking::SecretText;

use super::rx_edit::{EditError, EditProblem, RxEntryEdit, RxFieldEdit};
use super::rx_entry::{RxEntry, RxFieldSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditValue {
    Text(SecretText),
    Flag(bool),
    List(Vec<EditValue>),
    Map(HashMap<String, EditValue>),
}

impl EditValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(SecretText::new(value.into()))
    }
}

pub type EditValues = HashMap<String, EditValue>;

fn invalid(reason: String) -> EditError {
    EditError::Invalid(reason)
}

fn text(values: &EditValues, key: &str) -> Result<SecretText, EditError> {
    match values.get(key) {
        Some(EditValue::Text(value)) => Ok(value.clone()),
        _ => Err(invalid(format!("missing text value {key:?}"))),
    }
}

fn list<'a>(values: &'a EditValues, key: &str) -> Result<&'a [EditValue], EditError> {
    match values.get(key) {
        Some(EditValue::List(items)) => Ok(items),
        None => Ok(&[]),
        _ => Err(invalid(format!("value {key:?} is not a list"))),
    }
}

fn field_edit(item: &EditValue) -> Result<RxFieldEdit, EditError> {
    let EditValue::Map(map) = item else {
        return Err(invalid("custom field is not a map".into()));
    };
    let original = text(map, "originalName")?;
    let protected = match map.get("protected") {
        Some(EditValue::Flag(flag)) => *flag,
        _ => return Err(invalid("missing flag \"protected\"".into())),
    };
    Ok(RxFieldEdit {
        original_name: Some(original.to_string()).filter(|name| !name.is_empty()),
        name: text(map, "name")?.to_string(),
        value: text(map, "value")?,
        protected,
    })
}

fn is_blank_new_row(field: &RxFieldEdit) -> bool {
    field.original_name.is_none() && field.name.trim().is_empty() && field.value.is_empty()
}

impl RxEntryEdit {
    fn from_values_with_rows(
        uuid: Uuid,
        values: &EditValues,
    ) -> Result<(Self, Vec<usize>), EditError> {
        let rows = list(values, "customFields")?
            .iter()
            .map(field_edit)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .enumerate()
            .filter(|(_, field)| !is_blank_new_row(field))
            .collect::<Vec<_>>();
        let removed_fields = list(values, "removedFields")?
            .iter()
            .map(|item| match item {
                EditValue::Text(name) => Ok(name.to_string()),
                _ => Err(invalid("removed field is not text".into())),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let indexes = rows.iter().map(|(index, _)| *index).collect();
        let custom_fields = rows.into_iter().map(|(_, field)| field).collect();
        let edit = Self {
            uuid,
            title: text(values, "title")?,
            username: text(values, "username")?,
            password: text(values, "password")?,
            url: text(values, "url")?,
            notes: text(values, "notes")?,
            custom_fields,
            removed_fields,
        };
        Ok((edit, indexes))
    }

    pub fn from_values(uuid: Uuid, values: &EditValues) -> Result<Self, EditError> {
        Self::from_values_with_rows(uuid, values).map(|(edit, _)| edit)
    }

    pub fn values_over(&self, mut values: EditValues) -> EditValues {
        let text = |value: &str| EditValue::text(value);
        for (key, value) in [
            ("title", &self.title),
            ("username", &self.username),
            ("password", &self.password),
            ("url", &self.url),
            ("notes", &self.notes),
        ] {
            values.insert(key.to_string(), text(value));
        }
        let read_only = match values.remove("customFields") {
            Some(EditValue::List(fields)) => fields
                .into_iter()
                .filter(|field| {
                    matches!(field, EditValue::Map(map)
                        if map.get("editable") == Some(&EditValue::Flag(false)))
                })
                .collect(),
            _ => Vec::new(),
        };
        let edited = self.custom_fields.iter().map(|field| {
            EditValue::Map(HashMap::from([
                (
                    "originalName".to_string(),
                    text(field.original_name.as_deref().unwrap_or_default()),
                ),
                ("name".to_string(), text(&field.name)),
                ("value".to_string(), text(&field.value)),
                ("protected".to_string(), EditValue::Flag(field.protected)),
                ("editable".to_string(), EditValue::Flag(true)),
            ]))
        });
        values.insert(
            "customFields".to_string(),
            EditValue::List(edited.chain(read_only).collect()),
        );
        values.insert(
            "removedFields".to_string(),
            EditValue::List(self.removed_fields.iter().map(|name| text(name)).collect()),
        );
        values
    }
}

pub fn validate_edit_values(values: &EditValues) -> Vec<EditProblem> {
    let Ok((edit, indexes)) = RxEntryEdit::from_values_with_rows(Uuid::nil(), values) else {
        return Vec::new();
    };
    edit.problems()
        .into_iter()
        .filter_map(|problem| {
            let row = indexes.get(problem.index?)?;
            Some(EditProblem {
                index: Some(*row),
                message: problem.message,
            })
        })
        .collect()
}

fn value_text(value: Option<super::rx_entry::RxValueKeyRef<'_>>) -> EditValue {
    let text = value.and_then(|v| v.value()).unwrap_or_default();
    EditValue::Text(SecretText::new(text.to_string()))
}

impl RxEntry {
    pub fn edit_values(&self) -> EditValues {
        let custom_fields = self
            .custom_fields
            .iter_sourced()
            .map(|(name, value, source)| {
                let protected = value.is_hidden_by_default();
                EditValue::Map(HashMap::from([
                    ("name".to_string(), EditValue::text(name.as_str())),
                    ("value".to_string(), value_text(Some(value))),
                    ("protected".to_string(), EditValue::Flag(protected)),
                    (
                        "editable".to_string(),
                        EditValue::Flag(source == RxFieldSource::Field),
                    ),
                ]))
            })
            .collect();

        HashMap::from([
            ("uuid".to_string(), EditValue::text(self.uuid.to_string())),
            ("title".to_string(), value_text(self.title())),
            ("username".to_string(), value_text(self.username())),
            ("password".to_string(), value_text(self.password())),
            ("url".to_string(), value_text(self.url())),
            ("notes".to_string(), value_text(self.notes())),
            ("customFields".to_string(), EditValue::List(custom_fields)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(value: &str) -> EditValue {
        EditValue::text(value)
    }

    fn field(original: &str, name: &str, value: &str, protected: bool) -> EditValue {
        EditValue::Map(HashMap::from([
            ("originalName".to_string(), t(original)),
            ("name".to_string(), t(name)),
            ("value".to_string(), t(value)),
            ("protected".to_string(), EditValue::Flag(protected)),
        ]))
    }

    fn base() -> EditValues {
        HashMap::from([
            ("title".to_string(), t("Title")),
            ("username".to_string(), t("bob")),
            ("password".to_string(), t("secret")),
            ("url".to_string(), t("https://example.com")),
            ("notes".to_string(), t("")),
        ])
    }

    #[test]
    fn converts_all_keys() {
        let mut values = base();
        values.insert(
            "customFields".into(),
            EditValue::List(vec![
                field("PIN", "Code", "1234", true),
                field("", "New", "x", false),
            ]),
        );
        values.insert("removedFields".into(), EditValue::List(vec![t("Old")]));

        let uuid = Uuid::new_v4();
        let edit = RxEntryEdit::from_values(uuid, &values).unwrap();

        assert_eq!(edit.uuid, uuid);
        assert_eq!(edit.title.as_str(), "Title");
        assert_eq!(edit.password.as_str(), "secret");
        assert_eq!(edit.removed_fields, vec!["Old".to_string()]);
        assert_eq!(edit.custom_fields.len(), 2);
        assert_eq!(edit.custom_fields[0].original_name.as_deref(), Some("PIN"));
        assert_eq!(edit.custom_fields[0].name, "Code");
        assert!(edit.custom_fields[0].protected);
        assert_eq!(edit.custom_fields[1].original_name, None);
        assert!(!edit.custom_fields[1].protected);
    }

    #[test]
    fn values_over_restores_the_edit() {
        let mut values = base();
        let mut read_only = field("", "otp", "x", false);
        if let EditValue::Map(map) = &mut read_only {
            map.insert("editable".to_string(), EditValue::Flag(false));
        }
        values.insert(
            "customFields".into(),
            EditValue::List(vec![field("", "PIN", "1", true), read_only.clone()]),
        );
        let mut edited = base();
        edited.insert("password".into(), t("changed"));
        edited.insert(
            "customFields".into(),
            EditValue::List(vec![field("PIN", "Code", "2", true)]),
        );
        edited.insert("removedFields".into(), EditValue::List(vec![t("Old")]));
        let edit = RxEntryEdit::from_values(Uuid::nil(), &edited).unwrap();

        let over = edit.values_over(values);
        assert_eq!(over.get("password"), Some(&t("changed")));
        assert_eq!(
            over.get("removedFields"),
            Some(&EditValue::List(vec![t("Old")]))
        );
        let Some(EditValue::List(fields)) = over.get("customFields") else {
            panic!("no custom fields");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1], read_only);
        let mut expected = field("PIN", "Code", "2", true);
        if let EditValue::Map(map) = &mut expected {
            map.insert("editable".to_string(), EditValue::Flag(true));
        }
        assert_eq!(fields[0], expected);
    }

    #[test]
    fn custom_and_removed_fields_are_optional() {
        let edit = RxEntryEdit::from_values(Uuid::new_v4(), &base()).unwrap();
        assert!(edit.custom_fields.is_empty());
        assert!(edit.removed_fields.is_empty());
    }

    #[test]
    fn missing_text_value_fails() {
        let mut values = base();
        values.remove("password");
        assert!(RxEntryEdit::from_values(Uuid::new_v4(), &values).is_err());
    }

    #[test]
    fn malformed_field_fails() {
        let mut values = base();
        values.insert(
            "customFields".into(),
            EditValue::List(vec![EditValue::Map(HashMap::from([(
                "name".to_string(),
                t("x"),
            )]))]),
        );
        assert!(RxEntryEdit::from_values(Uuid::new_v4(), &values).is_err());
    }

    fn problems_of(fields: Vec<EditValue>) -> Vec<(usize, String)> {
        let mut values = base();
        values.insert("customFields".into(), EditValue::List(fields));
        validate_edit_values(&values)
            .into_iter()
            .map(|p| (p.index.unwrap(), p.message))
            .collect()
    }

    #[test]
    fn blank_new_row_is_dropped() {
        let mut values = base();
        values.insert(
            "customFields".into(),
            EditValue::List(vec![
                field("", "  ", "", false),
                field("", "Kept", "x", false),
                field("PIN", "", "", false),
            ]),
        );
        let edit = RxEntryEdit::from_values(Uuid::new_v4(), &values).unwrap();
        assert_eq!(edit.custom_fields.len(), 2);
        assert_eq!(edit.custom_fields[0].name, "Kept");
        assert_eq!(edit.custom_fields[1].original_name.as_deref(), Some("PIN"));
        assert!(problems_of(vec![field("", "", "", false)]).is_empty());
    }

    #[test]
    fn value_without_name_is_a_problem_at_its_row() {
        let problems = problems_of(vec![
            field("", "", "", false),
            field("", "Ok", "1", false),
            field("", "", "secret", false),
        ]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].0, 2);
    }

    #[test]
    fn duplicate_name_is_a_problem_on_the_second_row() {
        let problems = problems_of(vec![
            field("", "Dup", "1", false),
            field("", "Dup", "2", false),
        ]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].0, 1);
    }

    #[test]
    fn reserved_names_are_problems() {
        let problems = problems_of(vec![
            field("", "Password", "1", false),
            field("", "ok", "1", false),
            field("", "_etm_template_uuid", "1", false),
        ]);
        let rows: Vec<usize> = problems.iter().map(|p| p.0).collect();
        assert_eq!(rows, vec![0, 2]);
    }
}
