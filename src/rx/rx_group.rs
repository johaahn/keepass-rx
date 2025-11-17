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

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone, Hash, Eq, PartialEq)]
pub struct RxTemplate {
    #[zeroize(skip)]
    pub uuid: Uuid,
    pub name: String, // from the template's entry title.

    #[zeroize(skip)]
    pub entry_uuids: Vec<Uuid>,
}
