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

    pub showRecycleBin: qt_property!(bool; READ get_show_recycle_bin WRITE set_show_recycle_bin NOTIFY showRecycleBinChanged),
    pub showRecycleBinChanged: qt_signal!(),

    pub showAccents: qt_property!(bool; READ get_show_accents WRITE set_show_accents NOTIFY showAccentsChanged),
    pub showAccentsChanged: qt_signal!(),
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

            showRecycleBin: false,
            showRecycleBinChanged: Default::default(),

            showAccents: true,
            showAccentsChanged: Default::default(),
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

    fn value_bool(&self, key: &str) -> bool {
        self.inner.value_bool(key)
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.inner.set_bool(key, value);
    }

    pub fn get_show_accents(&self) -> bool {
        self.value_bool("showAccents")
    }

    pub fn set_show_accents(&mut self, value: bool) {
        self.set_bool("showAccents", value);
        self.showAccentsChanged();
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

        self.searchTypeChanged();
    }

    pub fn get_show_recycle_bin(&self) -> bool {
        self.value_bool("showRecycleBin")
    }

    pub fn set_show_recycle_bin(&mut self, value: bool) {
        self.set_bool("showRecycleBin", value);
        self.showRecycleBinChanged();
    }
}
