use std::collections::HashMap;

use uuid::Uuid;
use zeroize::Zeroizing;

use super::rx_edit::{EditError, RxEntryEdit, RxFieldEdit};
use super::rx_entry::{RxEntry, RxFieldSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditValue {
    Text(String),
    Flag(bool),
    List(Vec<EditValue>),
    Map(HashMap<String, EditValue>),
}

pub type EditValues = HashMap<String, EditValue>;

fn invalid(reason: String) -> EditError {
    EditError::Invalid(reason)
}

fn text(values: &EditValues, key: &str) -> Result<String, EditError> {
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
        original_name: Some(original).filter(|name| !name.is_empty()),
        name: text(map, "name")?,
        value: Zeroizing::new(text(map, "value")?),
        protected,
    })
}

impl RxEntryEdit {
    pub fn from_values(uuid: Uuid, values: &EditValues) -> Result<Self, EditError> {
        let custom_fields = list(values, "customFields")?
            .iter()
            .map(field_edit)
            .collect::<Result<Vec<_>, _>>()?;
        let removed_fields = list(values, "removedFields")?
            .iter()
            .map(|item| match item {
                EditValue::Text(name) => Ok(name.clone()),
                _ => Err(invalid("removed field is not text".into())),
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            uuid,
            title: Zeroizing::new(text(values, "title")?),
            username: Zeroizing::new(text(values, "username")?),
            password: Zeroizing::new(text(values, "password")?),
            url: Zeroizing::new(text(values, "url")?),
            notes: Zeroizing::new(text(values, "notes")?),
            custom_fields,
            removed_fields,
        })
    }
}

fn value_text(value: Option<super::rx_entry::RxValueKeyRef<'_>>) -> EditValue {
    let text = value.and_then(|v| v.value()).unwrap_or_default();
    EditValue::Text(text.to_string())
}

impl RxEntry {
    pub fn edit_values(&self) -> EditValues {
        let custom_fields = self
            .custom_fields
            .iter_sourced()
            .map(|(name, value, source)| {
                let protected = value.is_hidden_by_default();
                EditValue::Map(HashMap::from([
                    ("name".to_string(), EditValue::Text(name.clone())),
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
            ("uuid".to_string(), EditValue::Text(self.uuid.to_string())),
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
        EditValue::Text(value.to_string())
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
}
