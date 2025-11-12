use qmetaobject::{QString, QVariant, QVariantMap};
use std::collections::HashMap;

use crate::rx::{RxCustomFields, RxEntry, RxFieldName, RxValue, expose_opt};

use super::RxMetadata;

impl From<QString> for RxFieldName {
    fn from(value: QString) -> Self {
        RxFieldName::from(value.to_string())
    }
}

impl From<&RxEntry> for QVariantMap {
    fn from(value: &RxEntry) -> Self {
        enum ValueType<'a> {
            Rx(&'a Option<RxValue>),
            Uuid(uuid::Uuid),
        }

        let icon_data_url: QString =
            value.icon_data_url().map(QString::from).unwrap_or_default();

        let maybe_insert =
            |map: &mut HashMap<String, QVariant>, field_name: &str, value: ValueType| {
                match value {
                    ValueType::Rx(rx_val) => {
                        if let Some(rx_val_str) = expose_opt!(rx_val) {
                            map.insert(
                                field_name.into(),
                                QString::from(rx_val_str.to_string()).into(),
                            );
                        }
                    }
                    ValueType::Uuid(uuid) => {
                        map.insert(field_name.into(), QString::from(uuid.to_string()).into());
                    }
                }
            };

        let totp = value.totp();
        let mut map: HashMap<String, QVariant> = HashMap::new();

        maybe_insert(&mut map, "uuid", ValueType::Uuid(value.uuid));
        maybe_insert(&mut map, "url", ValueType::Rx(&value.url));
        maybe_insert(&mut map, "username", ValueType::Rx(&value.username));
        maybe_insert(&mut map, "title", ValueType::Rx(&value.title));
        maybe_insert(&mut map, "password", ValueType::Rx(&value.password));
        maybe_insert(&mut map, "notes", ValueType::Rx(&value.notes));

        map.insert("iconPath".to_string(), icon_data_url.into());
        map.insert(
            "customFields".to_string(),
            value.custom_fields.clone().into(),
        );

        if let Ok(_) = totp {
            map.insert("hasTotp".to_string(), true.into());
        } else {
            map.insert("hasTotp".to_string(), false.into());
        }

        map.into()
    }
}

impl From<RxEntry> for QVariantMap {
    fn from(value: RxEntry) -> QVariantMap {
        QVariantMap::from(&value)
    }
}

impl From<RxEntry> for QVariant {
    fn from(value: RxEntry) -> Self {
        Into::<QVariantMap>::into(value).into()
    }
}

impl From<RxValue> for QVariantMap {
    fn from(value: RxValue) -> Self {
        let mut map = QVariantMap::default();
        map.insert(
            "value".into(),
            value.value().map(QString::from).unwrap_or_default().into(),
        );

        map.insert(
            "isHiddenByDefault".into(),
            value.is_hidden_by_default().into(),
        );

        map
    }
}

impl From<RxCustomFields> for QVariantMap {
    fn from(value: RxCustomFields) -> QVariantMap {
        let mut map: HashMap<String, QVariant> = HashMap::new();

        for (key, value) in value.0 {
            let q_val = QVariantMap::from(value);
            map.insert(key, q_val.into());
        }

        map.into()
    }
}

impl From<RxCustomFields> for QVariant {
    fn from(value: RxCustomFields) -> QVariant {
        QVariantMap::from(value).into()
    }
}

impl From<&RxMetadata> for QVariantMap {
    fn from(value: &RxMetadata) -> Self {
        let mut map = QVariantMap::default();

        map.insert(
            "publicName".into(),
            value
                .name
                .as_ref()
                .map(|val| QString::from(val.as_str()))
                .unwrap_or_default()
                .into(),
        );

        map.insert("publicIcon".into(), value.icon.unwrap_or_default().into());

        map.insert(
            "publicColor".into(),
            value
                .color
                .as_ref()
                .map(|val| QString::from(val.as_str()))
                .unwrap_or_default()
                .into(),
        );

        map
    }
}
