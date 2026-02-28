use crate::license::{LICENSES, License};
use qmeta_async::with_executor;
use qmetaobject::{QVariantList, QVariantMap, prelude::*};

#[derive(QObject, Default)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiLicenses {
    pub(super) base: qt_base_class!(trait QObject),
    pub allLicenses: qt_method!(fn(&self) -> QVariantList),
}

macro_rules! qstr {
    ($value:expr) => {{ QVariant::from(QString::from($value.to_string())) }};
}
impl From<&License> for QVariantMap {
    fn from(value: &License) -> Self {
        let mut map = QVariantMap::default();

        map.insert("crateName".into(), qstr!(value.crate_name));
        map.insert("crateVersion".into(), qstr!(value.crate_version));
        map.insert("crateURL".into(), qstr!(value.crate_url));
        map.insert("licenseName".into(), qstr!(value.license_name));
        map.insert("licenseSPDX".into(), qstr!(value.license_spdx));
        map.insert("licenseText".into(), qstr!(value.license_text));

        map
    }
}

impl From<&License> for QVariant {
    fn from(value: &License) -> Self {
        QVariantMap::from(value).into()
    }
}

// impl from License to QVariantMap. this component just lists all the
// licenses. qt_property of allLicenses, which is a QVariantList. Then
// this can be fed to a list model.
#[allow(non_snake_case)]
impl RxUiLicenses {
    #[with_executor]
    pub fn allLicenses(&self) -> QVariantList {
        LICENSES
            .iter()
            .map(|license| QVariant::from(license))
            .collect()
    }
}
