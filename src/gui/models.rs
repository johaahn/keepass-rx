use anyhow::anyhow;
use gettextrs::ngettext;
use qmetaobject::{
    QMetaType, QObject, QString, QStringList, QVariant, QVariantList, QVariantMap,
};
use uuid::Uuid;

use crate::rx::{
    RxContainedRef, RxContainer, RxContainerGrouping, RxContainerItem, RxEntry, RxGroup,
    RxGrouping, RxMetadata, RxTag, RxTemplate,
};

#[derive(QEnum, Clone, Default, Copy)]
#[repr(C)]
pub enum RxItemType {
    #[default]
    Entry,
    Group,
    Template,
    Tag,
}

fn entry_type_from_string(qval: &QString) -> RxItemType {
    match qval.to_string().as_str() {
        "Group" => RxItemType::Group,
        "Entry" => RxItemType::Entry,
        "Template" => RxItemType::Template,
        "Tag" => RxItemType::Tag,
        _ => panic!("Invalid entry type: {}", qval),
    }
}

fn entry_type_to_string(entry_type: &RxItemType) -> QString {
    match entry_type {
        RxItemType::Group => "Group",
        RxItemType::Entry => "Entry",
        RxItemType::Template => "Template",
        RxItemType::Tag => "Tag",
    }
    .into()
}

impl QMetaType for RxItemType {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(entry_type_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(entry_type_to_string);
}

/// A setting that controls how an RxListItem is rendered in the UI.
/// Note that the UI container of the list item must also have the
/// feature enabled for the feature to be enabled. This prevents,
/// e.g., rendering 2FA codes in the list outside of the 2FA codes
/// view.
#[derive(QEnum, Clone, Default, Copy, PartialEq)]
#[repr(C)]
pub enum RxUiFeature {
    #[default]
    None,
    DisplayTwoFactorAuth,
}

impl std::fmt::Display for RxUiFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", ui_feature_to_string(self).to_string())
    }
}

fn ui_feature_from_string(qval: &QString) -> RxUiFeature {
    match qval.to_string().as_str() {
        "None" => RxUiFeature::None,
        "DisplayTwoFactorAuth" => RxUiFeature::DisplayTwoFactorAuth,
        _ => panic!("Not a UI feature: {}", qval),
    }
}

fn ui_feature_to_string(ui_feature: &RxUiFeature) -> QString {
    match ui_feature {
        RxUiFeature::None => "None",
        RxUiFeature::DisplayTwoFactorAuth => "DisplayTwoFactorAuth",
    }
    .into()
}

impl QMetaType for RxUiFeature {
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

/// Create translatable string for counting a list of entries.
fn entry_count<T>(entries: &[T]) -> QString {
    format!(
        "{} {}",
        entries.len(),
        ngettext(
            "entry",
            "entries",
            // Convert to usize without panicking
            entries.len().try_into().ok().unwrap_or(0u32)
        )
    )
    .into()
}

fn entry_count_len(len: usize) -> QString {
    format!(
        "{} {}",
        len,
        ngettext(
            "entry",
            "entries",
            // Convert to usize without panicking
            len.try_into().ok().unwrap_or(0u32)
        )
    )
    .into()
}

#[derive(QObject, Default)]
#[allow(dead_code, non_snake_case)]
pub struct RxListItem {
    pub(super) base: qt_base_class!(trait QObject),
    pub(super) itemType: qt_property!(RxItemType),

    pub(super) uuid: qt_property!(QString),
    pub(super) parentUuid: qt_property!(QString),
    pub(super) title: qt_property!(QString), // Large text
    pub(super) subtitle: qt_property!(QString), // Second line
    pub(super) description: qt_property!(QString), // Third line, description
    pub(super) iconPath: qt_property!(QString),
    pub(super) iconBuiltin: qt_property!(bool),

    // A "feature" that changes how the item is rendered. For example,
    // displaying a 2FA code.
    pub(super) feature: qt_property!(RxUiFeature),

    // Mostly for passwords entries. Does not really apply to groups.
    pub(super) hasUsername: qt_property!(bool),
    pub(super) hasPassword: qt_property!(bool),
    pub(super) hasURL: qt_property!(bool),
    pub(super) hasTOTP: qt_property!(bool),
}

impl RxListItem {
    pub fn for_virtual_root(name: String) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Group,
            uuid: QString::from(Uuid::default().to_string()),
            parentUuid: QString::default(),
            feature: RxUiFeature::None,
            description: QString::default(),

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,

            iconPath: QString::default(),
            iconBuiltin: false,
            title: QString::from(name.as_ref()),
            subtitle: QString::from(""),
        }
    }
}

impl From<RxContainedRef> for RxListItem {
    fn from(value: RxContainedRef) -> Self {
        match value {
            RxContainedRef::Entry(entry) => RxListItem::from(entry.as_ref()),
            RxContainedRef::Group(group) => RxListItem::from(group.as_ref()),
            RxContainedRef::Template(template) => RxListItem::from(template.as_ref()),
            RxContainedRef::Tag(tag) => RxListItem::from(&tag),
            RxContainedRef::VirtualRoot(root_name) => RxListItem::for_virtual_root(root_name),
        }
    }
}

impl From<&RxTag> for RxListItem {
    fn from(value: &RxTag) -> Self {
        RxListItem {
            base: Default::default(),
            itemType: RxItemType::Tag,
            uuid: QString::from(value.uuid.to_string()),
            parentUuid: QString::default(),
            feature: RxUiFeature::None,

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,

            iconPath: QString::default(),
            iconBuiltin: false,
            title: QString::from(value.name.as_ref()),
            subtitle: QString::from("Tag"),
            description: entry_count(&value.entry_uuids),
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
            feature: RxUiFeature::None,

            hasUsername: false,
            hasPassword: false,
            hasURL: false,
            hasTOTP: false,

            iconPath: value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default(),

            iconBuiltin: value.icon.is_builtin(),
            title: QString::from(value.name.as_ref()),
            subtitle: QString::from("Template"),
            description: entry_count(&value.entry_uuids),
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
            feature: if value.has_otp() {
                RxUiFeature::DisplayTwoFactorAuth
            } else {
                RxUiFeature::None
            },

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

            description: QString::from(match value.password() {
                Some(_) => "••••••",
                _ => "",
            }),
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
            feature: RxUiFeature::None,

            parentUuid: value
                .parent
                .map(|parent| QString::from(parent.to_string()))
                .unwrap_or_default(),

            title: value.name.clone().into(),
            subtitle: QString::from("Group"),
            description: entry_count_len(value.entries.len() + value.subgroups.len()),

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
        map.insert("description".into(), value.description.to_qvariant());
        map.insert("hasUsername".into(), value.hasUsername.to_qvariant());
        map.insert("hasPassword".into(), value.hasPassword.to_qvariant());
        map.insert("hasURL".into(), value.hasURL.to_qvariant());
        map.insert("hasTOTP".into(), value.hasTOTP.to_qvariant());
        map.insert("iconPath".into(), value.iconPath.to_qvariant());
        map.insert("iconBuiltin".into(), value.iconBuiltin.to_qvariant());
        map.insert("feature".into(), value.feature.to_qvariant());

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

/// What group/template container we are in. Used in conjunction with
/// RxViewMode to determine if we should be able to travel back up the
/// tree and so on.
#[derive(Default, Clone)]
pub struct RxUiContainer {
    pub uuid: Uuid,
    pub is_root: bool,
    pub available_feature: RxUiFeature,
    pub instructions: Option<String>,
}

impl From<&RxUiContainer> for QVariantMap {
    fn from(value: &RxUiContainer) -> Self {
        let mut qvar = QVariantMap::default();
        qvar.insert(
            "containerUuid".into(),
            QString::from(value.uuid.to_string()).into(),
        );

        qvar.insert("isRoot".into(), value.is_root.into());

        qvar.insert(
            "availableFeature".into(),
            ui_feature_to_string(&value.available_feature).into(),
        );

        qvar.insert(
            "instructions".into(),
            value
                .instructions
                .as_deref()
                .map(QString::from)
                .unwrap_or_default()
                .into(),
        );

        qvar
    }
}
