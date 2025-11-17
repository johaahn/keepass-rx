use anyhow::anyhow;
use qmetaobject::{QMetaType, QObject, QString, QVariant, QVariantList, QVariantMap};
use uuid::Uuid;

use crate::rx::{
    RxContainedRef, RxContainer, RxContainerGrouping, RxContainerItem, RxContainerWithDb,
    RxEntry, RxGroup, RxGroupingType, RxMetadata, RxTemplate,
};

#[derive(QEnum, Clone, Default, Copy)]
#[repr(C)]
pub enum RxItemType {
    #[default]
    Entry,
    Group,
    Template,
}

fn entry_type_from_string(qval: &QString) -> RxItemType {
    match qval.to_string().as_str() {
        "Group" => RxItemType::Group,
        "Entry" => RxItemType::Entry,
        "Template" => RxItemType::Template,
        _ => panic!("Invalid entry type: {}", qval),
    }
}

fn entry_type_to_string(entry_type: &RxItemType) -> QString {
    match entry_type {
        RxItemType::Group => "Group",
        RxItemType::Entry => "Entry",
        RxItemType::Template => "Template",
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
    iconBuiltin: qt_property!(bool),

    // Mostly for passwords entries. Does not really apply to groups.
    hasUsername: qt_property!(bool),
    hasPassword: qt_property!(bool),
    hasURL: qt_property!(bool),
    hasTOTP: qt_property!(bool),
}

impl From<RxContainedRef<'_>> for RxListItem {
    fn from(value: RxContainedRef<'_>) -> Self {
        match value {
            RxContainedRef::Entry(entry) => RxListItem::from(entry),
            RxContainedRef::Group(group) => RxListItem::from(group),
            RxContainedRef::Template(template) => RxListItem::from(template),
        }
    }
}

impl From<&RxTemplate> for RxListItem {
    fn from(value: &RxTemplate) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Template,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: QString::default(),

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,

            iconPath: QString::default(),
            iconBuiltin: false,
            title: QString::from(value.name.as_ref()),
            subtitle: QString::from(""),
        }
    }
}

impl From<&RxEntry> for RxListItem {
    fn from(value: &RxEntry) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Entry,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: QString::from(value.parent_group.to_string()),

            hasUsername: value.username().is_some(),
            hasPassword: value.password().is_some(),
            hasURL: value.url().is_some(),
            hasTOTP: value.raw_otp_value().is_some(),

            iconPath: value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default(),

            iconBuiltin: value.icon.is_builtin(),

            title: value
                .title()
                .and_then(|title| title.value().map(|t| t.to_string()))
                .unwrap_or_else(|| "(Untitled)".to_string())
                .into(),

            subtitle: value
                .username()
                .and_then(|username| username.value().map(|u| u.to_string()))
                .unwrap_or_else(|| "".to_string())
                .into(),
        }
    }
}

impl From<RxEntry> for RxListItem {
    fn from(value: RxEntry) -> Self {
        RxListItem::from(&value)
    }
}

impl From<&RxGroup> for RxListItem {
    fn from(value: &RxGroup) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Group,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: value
                .parent
                .map(|parent| QString::from(parent.to_string()))
                .unwrap_or_default(),

            title: value.name.clone().into(),
            subtitle: QString::from("Group"),
            iconPath: value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default(),

            iconBuiltin: value.icon.is_builtin(),

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,
        }
    }
}

impl From<RxGroup> for RxListItem {
    fn from(value: RxGroup) -> Self {
        RxListItem::from(&value)
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
        map.insert("iconBuiltin".into(), value.iconBuiltin.to_qvariant());

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

#[derive(Default, Clone, Copy)]
pub enum RxPageType {
    #[default]
    Group,
    Template,
}

impl TryFrom<&RxContainer> for RxPageType {
    type Error = anyhow::Error;
    fn try_from(value: &RxContainer) -> Result<Self, Self::Error> {
        match value.item().grouping_type() {
            Some(RxGroupingType::Group) => Ok(RxPageType::Group),
            Some(RxGroupingType::Template) => Ok(RxPageType::Template),
            _ => Err(anyhow!(
                "Not a thing that can be converted into a page type"
            )),
        }
    }
}

/// What group/template container we are in. Used in conjunction with
/// RxViewMode to determine if we should be able to travel back up the
/// tree and so on.
#[derive(Default, Clone)]
pub struct RxUiContainer {
    pub uuid: Uuid,
    pub page_type: RxPageType,
    pub is_root: bool,
}

impl ToString for RxPageType {
    fn to_string(&self) -> String {
        match self {
            RxPageType::Template => "Template".to_string(),
            RxPageType::Group => "Group".to_string(),
        }
    }
}

impl From<&RxUiContainer> for QVariantMap {
    fn from(value: &RxUiContainer) -> Self {
        let mut qvar = QVariantMap::default();
        qvar.insert(
            "containerUuid".into(),
            QString::from(value.uuid.to_string()).into(),
        );

        qvar.insert(
            "containerType".into(),
            QString::from(value.page_type.to_string()).into(),
        );

        qvar.insert("isRoot".into(), value.is_root.into());

        qvar
    }
}
