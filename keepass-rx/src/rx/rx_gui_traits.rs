use gettextrs::{gettext, pgettext};
use qmetaobject::{QMetaType, QString, QVariant, QVariantMap};
use std::collections::HashMap;

use crate::rx::{RxCustomFields, RxEntry, RxFieldName, entropy::PasswordQuality};

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
        let icon_data_url: QString =
            value.icon_data_url().map(QString::from).unwrap_or_default();
        let maybe_insert_plaintext =
            |map: &mut HashMap<String, QVariant>, field_name: &str, value: Option<RxValueKeyRef<'_>>| {
                if let Some(rx_val) = value
                    && !rx_val.is_hidden_by_default()
                    && let Some(rx_val_str) = rx_val.value()
                {
                    map.insert(field_name.into(), QString::from(rx_val_str.as_str()).into());
                }
            };

        let totp = value.totp();
        let mut map: HashMap<String, QVariant> = HashMap::new();

        map.insert("uuid".into(), QString::from(value.uuid.to_string()).into());

        if let Some(title) = value.title().and_then(|title| title.value()) {
            map.insert("title".into(), QString::from(title.as_str()).into());
        }

        map.insert("hasUsername".into(), value.username().is_some().into());
        map.insert("hasPassword".into(), value.password().is_some().into());
        map.insert("hasUrl".into(), value.url().is_some().into());
        map.insert("hasNotes".into(), value.notes().is_some().into());
        maybe_insert_plaintext(&mut map, "username", value.username());
        maybe_insert_plaintext(&mut map, "password", value.password());
        maybe_insert_plaintext(&mut map, "url", value.url());
        maybe_insert_plaintext(&mut map, "notes", value.notes());
        if value.password().is_some() {
            let entropy = value.entropy();
            map.insert("entropy".to_string(), QVariant::from(entropy));
            map.insert(
                "entropyQuality".to_string(),
                QString::from(PasswordQuality::from(entropy).to_string()).into(),
            );
        }

        map.insert("iconPath".to_string(), icon_data_url.into());
        map.insert(
            "customFields".to_string(),
            QVariantMap::from(&value.custom_fields).into(),
        );

        if let Ok(_) = totp {
            map.insert("hasTotp".to_string(), true.into());
        } else {
            map.insert("hasTotp".to_string(), false.into());
        }

        map.into()
    }
}

impl std::fmt::Display for PasswordQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // KeePassXC's password widget groups Bad and Poor as "Poor".
            PasswordQuality::Bad | PasswordQuality::Poor => {
                write!(f, "{}", pgettext("Password strength", "Poor"))
            }
            PasswordQuality::Weak => write!(f, "{}", pgettext("Password strength", "Weak")),
            PasswordQuality::Good => write!(f, "{}", pgettext("Password strength", "Good")),
            PasswordQuality::Excellent => {
                write!(f, "{}", pgettext("Password strength", "Excellent"))
            }
        }
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

impl From<&RxCustomFields> for QVariantMap {
    fn from(value: &RxCustomFields) -> QVariantMap {
        let mut map: HashMap<String, QVariant> = HashMap::new();

        for (key, value) in value.iter() {
            let mut q_val = QVariantMap::default();
            if !value.is_hidden_by_default()
                && let Some(plaintext) = value.value()
            {
                q_val.insert("value".into(), QString::from(plaintext.as_str()).into());
            }
            q_val.insert(
                "isHiddenByDefault".into(),
                value.is_hidden_by_default().into(),
            );
            map.insert(key.to_string(), q_val.into());
        }

        map.into()
    }
}

impl From<RxCustomFields> for QVariantMap {
    fn from(value: RxCustomFields) -> QVariantMap {
        QVariantMap::from(&value)
    }
}

impl From<RxCustomFields> for QVariant {
    fn from(value: RxCustomFields) -> QVariant {
        QVariantMap::from(&value).into()
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
