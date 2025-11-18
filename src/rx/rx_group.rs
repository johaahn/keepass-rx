use std::sync::atomic::{AtomicUsize, Ordering};

use keepass::db::Group;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::icons::RxIcon;

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
    pub fn new(
        group: &mut Group,
        subgroups: Vec<Uuid>,
        entries: Vec<Uuid>,
        parent: Option<Uuid>,
    ) -> Self {
        let icon = match (group.custom_icon_uuid, group.icon_id) {
            (Some(_custom_id), _) => RxIcon::None, // TODO support custom group icons
            (_, Some(buitin_id)) => RxIcon::Builtin(buitin_id),
            _ => RxIcon::None,
        };

        Self {
            uuid: group.uuid,
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

static TAG_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

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
