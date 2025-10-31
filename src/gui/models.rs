use qmetaobject::{QMetaType, QObject, QString, QVariant, QVariantList, QVariantMap};

use crate::rx::{RxEntry, RxGroup};
use secrecy::ExposeSecret;

#[derive(QEnum, Clone, Default, Copy)]
#[repr(C)]
pub enum RxItemType {
    #[default]
    Entry,
    Group,
    GoBack,
}

fn entry_type_from_string(qval: &QString) -> RxItemType {
    match qval.to_string().as_str() {
        "Group" => RxItemType::Group,
        "Entry" => RxItemType::Entry,
        "GoBack" => RxItemType::GoBack,
        _ => panic!("Invalid entry type: {}", qval),
    }
}

fn entry_type_to_string(entry_type: &RxItemType) -> QString {
    match entry_type {
        RxItemType::Group => "Group",
        RxItemType::Entry => "Entry",
        RxItemType::GoBack => "GoBack",
    }
    .into()
}

impl QMetaType for RxItemType {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(entry_type_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(entry_type_to_string);
}

#[derive(QObject, Default)]
#[allow(dead_code, non_snake_case)]
pub struct RxListItem {
    base: qt_base_class!(trait QObject),
    itemType: qt_property!(RxItemType),

    uuid: qt_property!(QString),
    parentUuid: qt_property!(QString),
    title: qt_property!(QString),
    subtitle: qt_property!(QString),
    iconPath: qt_property!(QString),

    // Mostly for passwords entries. Does not really apply to groups.
    hasUsername: qt_property!(bool),
    hasPassword: qt_property!(bool),
    hasURL: qt_property!(bool),
    hasTOTP: qt_property!(bool),
}

impl RxListItem {
    /// Create a list item that represents going back up the group
    /// structure. TODO to be replaced with breadcrumb nav thingy.
    pub fn go_back() -> Self {
        Self {
            itemType: RxItemType::GoBack,
            title: "..".to_string().into(),
            ..Default::default()
        }
    }
}

impl From<&RxEntry> for RxListItem {
    fn from(value: &RxEntry) -> Self {
        value.clone().into()
    }
}

impl From<RxEntry> for RxListItem {
    fn from(value: RxEntry) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Entry,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: QString::from(value.parent_group.to_string()),

            hasUsername: value.username.is_some(),
            hasPassword: value.password.is_some(),
            hasURL: value.url.is_some(),
            hasTOTP: value.raw_otp_value.is_some(),

            iconPath: value.icon_data_url().map(QString::from).unwrap_or_default(),

            title: value
                .title
                .as_ref()
                .map(|title| title.expose_secret().to_string())
                .unwrap_or_else(|| "(Untitled)".to_string())
                .into(),

            subtitle: value
                .username
                .as_ref()
                .map(|username| username.expose_secret().to_string())
                .unwrap_or_else(|| "".to_string())
                .into(),
        }
    }
}

impl From<&RxGroup> for RxListItem {
    fn from(value: &RxGroup) -> Self {
        value.clone().into()
    }
}

impl From<RxGroup> for RxListItem {
    fn from(value: RxGroup) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Group,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: value
                .parent
                .map(|parent| QString::from(parent.to_string()))
                .unwrap_or_default(),

            title: value.name.into(),
            subtitle: QString::from("Group"),

            // TODO support group icons
            iconPath: QString::default(),

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,
        }
    }
}

impl From<RxListItem> for QVariant {
    fn from(value: RxListItem) -> Self {
        QVariantMap::from(value).to_qvariant()
    }
}

impl From<RxListItem> for QVariantMap {
    fn from(value: RxListItem) -> Self {
        let mut map = QVariantMap::default();

        // TODO would be better to directly pass the enum as a
        // qvariant, but need to figure out how to compare in QML
        // first.
        map.insert(
            "itemType".to_string().into(),
            entry_type_to_string(&value.itemType).to_qvariant(),
        );

        map.insert("uuid".into(), value.uuid.to_qvariant());
        map.insert("title".into(), value.title.to_qvariant());
        map.insert("subtitle".into(), value.subtitle.to_qvariant());
        map.insert("hasUsername".into(), value.hasUsername.to_qvariant());
        map.insert("hasPassword".into(), value.hasPassword.to_qvariant());
        map.insert("hasURL".into(), value.hasURL.to_qvariant());
        map.insert("hasTOTP".into(), value.hasTOTP.to_qvariant());
        map.insert("iconPath".into(), value.iconPath.to_qvariant());

        map
    }
}

pub struct RxList(Vec<RxListItem>);

impl From<RxList> for QVariantList {
    fn from(value: RxList) -> Self {
        let RxList(items) = value;
        items.into_iter().map(|item| QVariant::from(item)).collect()
    }
}
