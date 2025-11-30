use qmetaobject::QVariantMap;

pub struct License {
    pub crate_name: &'static str,
    pub crate_version: &'static str,
    pub crate_url: &'static str,
    pub license_name: &'static str,
    pub license_spdx: &'static str,
    pub license_text: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/licenses.rs"));
