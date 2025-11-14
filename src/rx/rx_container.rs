use keepass::db::Group;
use std::mem;
use uuid::Uuid;
use zeroize::Zeroize;

/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use super::{RxDatabase, RxEntry, RxValue, icons::RxIcon};

pub enum RxContainer<'a> {
    Group(&'a RxGroup),
    Template(&'a RxTemplate),
    Entry(&'a RxEntry),
}

#[allow(dead_code)]
impl RxContainer<'_> {
    pub fn uuid(&self) -> Uuid {
        match self {
            RxContainer::Group(group) => group.uuid,
            RxContainer::Template(template) => template.uuid,
            RxContainer::Entry(entry) => entry.uuid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            RxContainer::Group(group) => &group.name,
            RxContainer::Template(template) => &template.name,
            RxContainer::Entry(entry) => entry
                .title
                .as_ref()
                .and_then(|v| {
                    if let RxValue::Unprotected(val) = v {
                        Some(val.as_ref())
                    } else {
                        None
                    }
                })
                .unwrap_or("Unknown"),
        }
    }

    pub fn children(&self) -> Vec<Uuid> {
        match self {
            RxContainer::Group(group) => {
                [group.subgroups.as_slice(), group.entries.as_slice()].concat()
            }
            RxContainer::Template(tmplt) => tmplt.entry_uuids.clone(),
            RxContainer::Entry(_) => vec![],
        }
    }
}

// TODO maybe make this the actual hierarchy struct/enum. Or maybe impl Hierarchy<RxTemplate> for RxContainerWithDb?
pub struct RxContainerWithDb<'db>(RxContainer<'db>, &'db RxDatabase);

impl<'db> RxContainerWithDb<'db> {
    pub fn is_root(&self) -> bool {
        match self.0 {
            RxContainer::Group(group) => group.uuid == self.1.root_group().uuid,
            RxContainer::Template(tmplt) => tmplt.uuid == Uuid::default(), // Virtual root
            RxContainer::Entry(_) => false, // An entry is always part of some group.
        }
    }

    pub fn children(&self) -> Vec<RxContainer<'db>> {
        self.0
            .children()
            .into_iter()
            .flat_map(|child_uuid| self.1.get_container(child_uuid))
            .collect()
    }

    pub fn find_children(&self, search_term: Option<&str>) -> Vec<RxContainer<'db>> {
        match self.0 {
            RxContainer::Template(_) if self.is_root() => self
                .1
                .find_templates(search_term)
                .map(|tmplt| RxContainer::Template(tmplt))
                .collect(),
            RxContainer::Template(tmplt) if !self.is_root() => self
                .1
                .get_template(tmplt.uuid)
                .map(|template| {
                    self.1
                        .entries_iter_by_uuid(template.entry_uuids.as_slice(), search_term)
                        .map(|ent| RxContainer::Entry(ent))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            RxContainer::Group(group) => {
                let subgroups_iter = self.1.filter_subgroups(group.uuid, search_term);
                let entries = self.1.get_entries(group.uuid, search_term);

                // Groups first, then entries below.
                let mut item_list: Vec<RxContainer<'_>> = subgroups_iter
                    .map(|subgroup| RxContainer::Group(subgroup))
                    .collect();

                item_list.append(
                    &mut entries
                        .into_iter()
                        .map(|ent| RxContainer::Entry(ent))
                        .collect(),
                );

                item_list
            }
            _ => vec![],
        }
    }
}

#[derive(Zeroize, Default, Clone)]
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
            name: mem::take(&mut group.name),
            subgroups: subgroups,
            entries: entries,
            parent: parent,
            icon: icon,
        }
    }
}

#[derive(Zeroize, Default, Clone, Hash, Eq, PartialEq)]
pub struct RxTemplate {
    #[zeroize(skip)]
    pub uuid: Uuid,
    pub name: String, // from the template's entry title.

    #[zeroize(skip)]
    pub entry_uuids: Vec<Uuid>,
}
