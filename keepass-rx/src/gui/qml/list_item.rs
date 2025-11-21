use paste::paste;
use std::str::FromStr;

use actor_macro::observing_model;
use gettextrs::npgettext;
use qmetaobject::{QMetaType, QObject, QPointer, QString};
use uuid::Uuid;

use crate::rx::virtual_hierarchy::{RxViewFeature, VirtualHierarchy};
use crate::rx::{RxContainedRef, RxEntry, RxGroup, RxTag, RxTemplate};

#[derive(QEnum, Clone, Default, Copy, PartialEq, Eq)]
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

/// Create translatable string for counting a list of entries.
fn entry_count<T>(entries: &[T]) -> QString {
    format!(
        "{} {}",
        entries.len(),
        npgettext(
            "Count of password entries in a group/folder in the main list.",
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
        npgettext(
            "Count of password entries in a group/folder in the main list.",
            "entry",
            "entries",
            // Convert to usize without panicking
            len.try_into().ok().unwrap_or(0u32)
        )
    )
    .into()
}

#[observing_model]
#[derive(QObject, Default)]
#[allow(dead_code, non_snake_case)]
pub struct RxListItem {
    pub(super) base: qt_base_class!(trait QObject),
    pub(super) itemType: qt_property!(RxItemType; NOTIFY itemTypeChanged),

    pub(super) entryUuid: qt_property!(QString; NOTIFY entryUuidChanged),
    pub(super) parentUuid: qt_property!(QString; NOTIFY parentUuidChanged),
    pub(super) title: qt_property!(QString; NOTIFY titleChanged),
    pub(super) subtitle: qt_property!(QString; NOTIFY subtitleChanged),
    pub(super) description: qt_property!(QString; NOTIFY descriptionChanged),
    pub(super) url: qt_property!(QString; NOTIFY urlChanged),
    pub(super) iconPath: qt_property!(QString; NOTIFY iconPathChanged),
    pub(super) iconBuiltin: qt_property!(bool; NOTIFY iconBuiltinChanged),

    // A "feature" that changes how the item is rendered. For example,
    // displaying a 2FA code.
    pub(super) feature: qt_property!(RxViewFeature; NOTIFY featureChanged),

    // Mostly for password entries. Does not really apply to groups.
    pub(super) hasUsername: qt_property!(bool; NOTIFY hasUsernameChanged),
    pub(super) hasPassword: qt_property!(bool; NOTIFY hasPasswordChanged),
    pub(super) hasURL: qt_property!(bool; NOTIFY hasURLChanged),
    pub(super) hasTOTP: qt_property!(bool; NOTIFY hasTOTPChanged),

    // Signals boilerplate
    itemTypeChanged: qt_signal!(),
    entryUuidChanged: qt_signal!(),
    parentUuidChanged: qt_signal!(),
    titleChanged: qt_signal!(),
    subtitleChanged: qt_signal!(),
    descriptionChanged: qt_signal!(),
    urlChanged: qt_signal!(),
    iconPathChanged: qt_signal!(),
    iconBuiltinChanged: qt_signal!(),
    featureChanged: qt_signal!(),
    hasUsernameChanged: qt_signal!(),
    hasPasswordChanged: qt_signal!(),
    hasURLChanged: qt_signal!(),
    hasTOTPChanged: qt_signal!(),
}

trait InitFrom<T> {
    fn init_from(&mut self, value: T);
}

macro_rules! set_value {
    ($self:ident.$field:ident, $value:expr) => {
        if $self.$field != $value {
            $self.$field = $value;
            paste! {
                $self.[<$field Changed>]();
            }
        }
    };
}

impl RxListItem {
    fn init_from_view(&mut self, view: &dyn VirtualHierarchy) {
        let mut loader = || -> anyhow::Result<()> {
            let uuid = Uuid::from_str(&self.entryUuid.to_string())?;

            // TODO maybe we should change this to not fire changed
            // events until the end, due to the feature being set or
            // not.
            self.init_from(view.get(uuid).expect("No container found"));

            // Only allow available feature of list item if the
            // current UI container has it enabled.
            if view.feature() != self.feature {
                //println!("WARN: Disabling unavailable UI feature in current container");
                self.feature = RxViewFeature::None;
                self.featureChanged();
            }
            Ok(())
        };

        loader().expect("Unable to load from current view");
    }

    pub fn init_from_virtual_root(&mut self, root_name: String) {
        set_value!(self.itemType, RxItemType::Group);
        set_value!(self.entryUuid, QString::from(Uuid::default().to_string()));
        set_value!(self.parentUuid, QString::default());
        set_value!(self.feature, RxViewFeature::None);
        set_value!(self.description, QString::default());

        set_value!(self.hasUsername, false);
        set_value!(self.hasPassword, false);
        set_value!(self.hasURL, false);
        set_value!(self.hasTOTP, false);

        set_value!(self.iconPath, QString::default());
        set_value!(self.iconBuiltin, false);
        set_value!(self.title, QString::from(root_name.as_ref()));
        set_value!(self.subtitle, QString::from(""));
    }
}

impl InitFrom<RxContainedRef> for RxListItem {
    fn init_from(&mut self, value: RxContainedRef) {
        match value {
            RxContainedRef::Entry(entry) => self.init_from(entry.as_ref()),
            RxContainedRef::Group(group) => self.init_from(group.as_ref()),
            RxContainedRef::Template(template) => self.init_from(template.as_ref()),
            RxContainedRef::Tag(tag) => self.init_from(&tag),
            RxContainedRef::VirtualRoot(root_name) => self.init_from_virtual_root(root_name),
        }
    }
}

impl InitFrom<&RxTag> for RxListItem {
    fn init_from(&mut self, value: &RxTag) {
        set_value!(self.itemType, RxItemType::Tag);
        set_value!(self.entryUuid, QString::from(value.uuid.to_string()));
        set_value!(self.parentUuid, QString::default());
        set_value!(self.feature, RxViewFeature::None);

        set_value!(self.hasUsername, false);
        set_value!(self.hasPassword, false);
        set_value!(self.hasURL, false);
        set_value!(self.hasTOTP, false);

        set_value!(self.iconPath, QString::default());
        set_value!(self.iconBuiltin, false);
        set_value!(self.title, QString::from(value.name.as_ref()));
        set_value!(self.subtitle, QString::from("Tag"));
        set_value!(self.description, entry_count(&value.entry_uuids));
    }
}

impl InitFrom<&RxTemplate> for RxListItem {
    fn init_from(&mut self, value: &RxTemplate) {
        set_value!(self.itemType, RxItemType::Template);
        set_value!(self.entryUuid, QString::from(value.uuid.to_string()));
        set_value!(self.parentUuid, QString::default());
        set_value!(self.feature, RxViewFeature::None);

        set_value!(self.hasUsername, false);
        set_value!(self.hasPassword, false);
        set_value!(self.hasURL, false);
        set_value!(self.hasTOTP, false);

        set_value!(
            self.iconPath,
            value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default()
        );

        set_value!(self.iconBuiltin, value.icon.is_builtin());
        set_value!(self.title, QString::from(value.name.as_ref()));
        set_value!(self.subtitle, QString::from("Template"));
        set_value!(self.description, entry_count(&value.entry_uuids));
    }
}

impl InitFrom<&RxEntry> for RxListItem {
    fn init_from(&mut self, value: &RxEntry) {
        set_value!(self.itemType, RxItemType::Entry);
        set_value!(self.entryUuid, QString::from(value.uuid.to_string()));
        set_value!(
            self.parentUuid,
            QString::from(value.parent_group.to_string())
        );
        set_value!(
            self.feature,
            if value.has_otp() {
                RxViewFeature::DisplayTwoFactorAuth
            } else {
                RxViewFeature::None
            }
        );

        set_value!(self.hasUsername, value.username().is_some());
        set_value!(self.hasPassword, value.password().is_some());
        set_value!(self.hasURL, value.url().is_some());
        set_value!(self.hasTOTP, value.raw_otp_value().is_some());

        set_value!(
            self.iconPath,
            value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default()
        );

        set_value!(self.iconBuiltin, value.icon.is_builtin());

        set_value!(
            self.url,
            value
                .url()
                .and_then(|url| url.value().map(|u| u.to_string()))
                .map(QString::from)
                .unwrap_or_default()
        );

        set_value!(
            self.title,
            value
                .title()
                .and_then(|title| title.value().map(|t| t.to_string()))
                .unwrap_or_else(|| "(Untitled)".to_string())
                .into()
        );

        set_value!(
            self.subtitle,
            value
                .username()
                .and_then(|username| username.value().map(|u| u.to_string()))
                .unwrap_or_else(|| "".to_string())
                .into()
        );

        set_value!(
            self.description,
            QString::from(match value.password() {
                Some(_) => "••••••",
                _ => "",
            })
        );
    }
}

impl InitFrom<RxEntry> for RxListItem {
    fn init_from(&mut self, value: RxEntry) {
        self.init_from(&value);
    }
}

impl InitFrom<&RxGroup> for RxListItem {
    fn init_from(&mut self, value: &RxGroup) {
        set_value!(self.itemType, RxItemType::Group);
        set_value!(self.entryUuid, QString::from(value.uuid.to_string()));
        set_value!(self.feature, RxViewFeature::None);

        set_value!(
            self.parentUuid,
            value
                .parent
                .map(|parent| QString::from(parent.to_string()))
                .unwrap_or_default()
        );

        set_value!(self.title, value.name.clone().into());
        set_value!(self.subtitle, QString::from("Group"));
        set_value!(
            self.description,
            entry_count_len(value.entries.len() + value.subgroups.len())
        );

        set_value!(
            self.iconPath,
            value
                .icon
                .icon_path()
                .map(QString::from)
                .unwrap_or_default()
        );

        set_value!(self.iconBuiltin, value.icon.is_builtin());

        set_value!(self.hasUsername, false);
        set_value!(self.hasPassword, false);
        set_value!(self.hasURL, false);
        set_value!(self.hasTOTP, false);
    }
}

impl InitFrom<RxGroup> for RxListItem {
    fn init_from(&mut self, value: RxGroup) {
        self.init_from(&value);
    }
}
