use qmetaobject::QObject;
use qttypes::QSettings;

use crate::rx::RxSearchType;

#[derive(QObject)]
#[allow(non_snake_case, dead_code)]
pub struct SettingsBridge {
    base: qt_base_class!(trait QObject),
    inner: QSettings,
    pub searchType: qt_property!(RxSearchType; READ get_search_type WRITE set_search_type NOTIFY searchTypeChanged),
    pub searchTypeChanged: qt_signal!(),
}

impl Default for SettingsBridge {
    fn default() -> Self {
        Self {
            base: Default::default(),
            inner: QSettings::from_path(
                dirs::config_dir()
                    .expect("Could not get xdg config directory path")
                    .join("keepassrx.projectmoon")
                    .join("keepassrx.projectmoon.conf")
                    .to_str()
                    .unwrap(),
            ),

            searchType: RxSearchType::CaseInsensitive,
            searchTypeChanged: Default::default(),
        }
    }
}

impl SettingsBridge {
    fn value_string(&self, key: &str) -> String {
        self.inner.value_string(key)
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        self.inner.set_string(key, value);
    }

    pub fn get_search_type(&self) -> RxSearchType {
        let search_type = self.value_string("searchType");
        match search_type.as_str() {
            "CaseInsensitive" => RxSearchType::CaseInsensitive,
            "Fuzzy" => RxSearchType::Fuzzy,
            _ => RxSearchType::default(),
        }
    }

    pub fn set_search_type(&mut self, search_type: RxSearchType) {
        match search_type {
            RxSearchType::CaseInsensitive => {
                self.set_string("searchType".into(), "CaseInsensitive".into())
            }
            RxSearchType::Fuzzy => self.set_string("searchType".into(), "Fuzzy".into()),
        }
    }
}
