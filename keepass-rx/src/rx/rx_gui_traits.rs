use qmetaobject::{QMetaType, QString, QVariant, QVariantMap};
use std::collections::HashMap;

use crate::rx::{RxCustomFields, RxEntry, RxFieldName, RxValue};

use super::{RxMetadata, RxValueKeyRef, virtual_hierarchy::RxViewFeature};

fn ui_feature_from_string(qval: &QString) -> RxViewFeature {
    match qval.to_string().as_str() {
        "None" => RxViewFeature::None,
        "DisplayTwoFactorAuth" => RxViewFeature::DisplayTwoFactorAuth,
        _ => panic!("Not a UI feature: {}", qval),
    }
}

fn ui_feature_to_string(ui_feature: &RxViewFeature) -> QString {
    match ui_feature {
        RxViewFeature::None => "None",
        RxViewFeature::DisplayTwoFactorAuth => "DisplayTwoFactorAuth",
    }
    .into()
}

impl QMetaType for RxViewFeature {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(ui_feature_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(ui_feature_to_string);

    fn to_qvariant(&self) -> QVariant {
        QVariant::from(ui_feature_to_string(self))
    }

    fn from_qvariant(variant: QVariant) -> Option<Self> {
        let qstr = variant.to_qstring();

        if !qstr.is_null() && !qstr.is_empty() {
            Some(ui_feature_from_string(&qstr))
        } else {
            None
        }
    }
}

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
            |map: &mut HashMap<String, QVariant>, field_name: &str, value_type: ValueType| {
                match value_type {
                    ValueType::Rx(rx_val) => {
                        let maybe_rx_val_str =
                            rx_val.as_ref().and_then(|v| v.value(value.master_key()));
                        if let Some(rx_val_str) = maybe_rx_val_str {
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

impl From<RxValueKeyRef<'_>> for QVariantMap {
    fn from(value: RxValueKeyRef<'_>) -> Self {
        let mut map = QVariantMap::default();
        map.insert(
            "value".into(),
            value
                .value()
                .map(|v| QString::from(v.as_str()))
                .unwrap_or_default()
                .into(),
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

        for (key, value) in value.iter() {
            let q_val = QVariantMap::from(value);
            map.insert(key.to_string(), q_val.into());
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

        // Standard KeePass Settings
        map.insert(
            "recycleBinUuid".into(),
            value
                .recycle_bin_uuid
                .map(|uuid| QString::from(uuid.to_string()))
                .unwrap_or_default()
                .into(),
        );

        // KeePassXC Settings
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

impl From<&RxMetadata> for QVariant {
    fn from(value: &RxMetadata) -> Self {
        QVariantMap::from(value).into()
    }
}
