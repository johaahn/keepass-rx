use keepass::db::{Group, GroupMut, Icon};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::icons::RxIcon;

pub struct RxGroupPart {
    name: String,
    icon: RxIcon,
    uuid: Uuid,
}

pub fn to_part<'db>(group: &mut GroupMut<'db>) -> RxGroupPart {
    let icon = match group.icon() {
        Some(Icon::BuiltIn(builtin_id)) => RxIcon::Builtin(*builtin_id),
        Some(Icon::Custom(_custom_icon_id)) => RxIcon::None, // TODO support custom group icons
        _ => RxIcon::None,
    };

    RxGroupPart {
        name: std::mem::take(&mut group.name),
        icon: icon,
        uuid: group.id().uuid(),
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxGroup {
    #[zeroize(skip)]
    pub uuid: Uuid,

    /// The parent UUID will be None if this is the root group.
    #[zeroize(skip)]
    pub parent: Option<Uuid>,

    pub name: String,

    #[zeroize(skip)]
    pub icon: RxIcon,

    #[zeroize(skip)]
    pub subgroups: Vec<Uuid>,

    #[zeroize(skip)]
    pub entries: Vec<Uuid>,
}

impl RxGroup {
    pub fn new<'db>(
        mut group: GroupMut<'db>,
        subgroups: Vec<Uuid>,
        entries: Vec<Uuid>,
        parent: Option<Uuid>,
    ) -> Self {
        let icon = match group.icon() {
            Some(Icon::BuiltIn(builtin_id)) => RxIcon::Builtin(*builtin_id),
            Some(Icon::Custom(_custom_icon_id)) => RxIcon::None, // TODO support custom group icons
            _ => RxIcon::None,
        };

        Self {
            uuid: group.id().uuid(),
            name: std::mem::take(&mut group.name),
            subgroups: subgroups,
            entries: entries,
            parent: parent,
            icon: icon,
        }
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxTemplate {
    #[zeroize(skip)]
    pub uuid: Uuid,
    pub name: String, // from the template's entry title.

    #[zeroize(skip)]
    pub icon: RxIcon,

    #[zeroize(skip)]
    pub entry_uuids: Vec<Uuid>,
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxTag {
    #[zeroize(skip)]
    pub(crate) uuid: Uuid,
    pub(crate) name: String,
    #[zeroize(skip)]
    pub(crate) entry_uuids: Vec<Uuid>,
}

impl RxTag {
    pub fn new(name: String, entry_uuids: Vec<Uuid>) -> Self {
        Self {
            uuid: Uuid::new_v4(), // TODO check for collision?
            name: name,
            entry_uuids: entry_uuids,
        }
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxSavedSearch {
    #[zeroize(skip)]
    pub(crate) uuid: Uuid,
    pub(crate) name: String,
    pub(crate) query: String,
    #[zeroize(skip)]
    pub(crate) entry_uuids: Vec<Uuid>,
}

impl RxSavedSearch {
    pub fn new(name: String, query: String, entry_uuids: Vec<Uuid>) -> Self {
        let stable_id = format!("{name}\n{query}");
        Self {
            uuid: Uuid::new_v5(&Uuid::NAMESPACE_OID, stable_id.as_bytes()),
            name,
            query,
            entry_uuids,
        }
    }
}
