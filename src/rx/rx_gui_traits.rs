use qmetaobject::{QString, QVariant, QVariantMap};
use std::collections::HashMap;

use crate::rx::{RxCustomFields, RxEntry, RxFieldName, RxValue, expose_opt};

impl From<QString> for RxFieldName {
    fn from(value: QString) -> Self {
        RxFieldName::from(value.to_string())
    }
}

impl From<&RxEntry> for QVariantMap {
    fn from(value: &RxEntry) -> Self {
        let icon_data_url: QString =
            value.icon_data_url().map(QString::from).unwrap_or_default();

        let mapper = |value: &Option<RxValue>| {
            expose_opt!(value)
                .map(|secret| QString::from(secret))
                .unwrap_or_default()
        };

        let totp = value.totp();

        let uuid: QString = value.uuid.to_string().into();
        let url: QString = mapper(&value.url);
        let username: QString = mapper(&value.username);
        let title: QString = mapper(&value.title);
        let password: QString = mapper(&value.password);
        let notes: QString = mapper(&value.notes);

        let mut map: HashMap<String, QVariant> = HashMap::new();
        map.insert("uuid".to_string(), uuid.into());
        map.insert("url".to_string(), url.into());
        map.insert("username".to_string(), username.into());
        map.insert("title".to_string(), title.into());
        map.insert("password".to_string(), password.into());
        map.insert("notes".to_string(), notes.into());
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

impl From<RxCustomFields> for QVariantMap {
    fn from(value: RxCustomFields) -> QVariantMap {
        let mut map: HashMap<String, QVariant> = HashMap::new();

        for (key, value) in value.0 {
            let q_val = value.value().map(QString::from).unwrap_or_default();
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
