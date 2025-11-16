use keepass::db::Group;
use std::{collections::HashMap, mem};
use uuid::Uuid;
use zeroize::Zeroize;

/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use super::{RxDatabase, RxEntry, RxGroup, RxTemplate, RxValue, icons::RxIcon};

pub struct RxRoot {
    root: RxContainer,
    children: Vec<RxContainer>,
}

impl RxRoot {
    pub fn virtual_root() -> Self {
        Self {
            children: vec![],
            root: RxContainer {
                item: RxContainerItem::VirtualRoot,
                is_root: true,
            },
        }
    }

    pub fn children(&self) -> &[RxContainer] {
        self.children.as_slice()
    }

    pub fn with_db<'root, 'db>(&'root self, db: &'db RxDatabase) -> RxRootWithDb<'root, 'db> {
        RxRootWithDb(self, db)
    }
}

#[derive(Clone)]
pub struct RxContainer {
    item: RxContainerItem,
    is_root: bool,
}

impl RxContainer {
    pub fn from<T>(item: T, is_root: bool) -> Self
    where
        T: Into<RxContainerItem>,
    {
        Self {
            item: item.into(),
            is_root,
        }
    }

    pub fn is_root(&self) -> bool {
        self.is_root
    }

    pub fn uuid(&self) -> Uuid {
        self.item.uuid()
    }

    pub fn item(&self) -> &RxContainerItem {
        &self.item
    }
}

#[derive(Default, Clone)]
pub enum RxContainerItem {
    /// VirtualRoot is for containers that don't have a clear actual
    /// existing root, e.g. list of all templates has no parent that
    /// could be root.
    #[default]
    VirtualRoot,
    Grouping(RxContainerGrouping),
    Entry(Uuid),
}

impl RxContainerItem {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Grouping(grouping) => grouping.uuid(),
            Self::Entry(entry_uuid) => *entry_uuid,
            Self::VirtualRoot => Uuid::default(),
        }
    }

    pub fn grouping_type(&self) -> Option<RxGroupingType> {
        match self {
            Self::Grouping(grouping) => Some(grouping.grouping_type()),
            Self::Entry(_) => None,
            Self::VirtualRoot => Some(RxGroupingType::Root),
        }
    }
}

impl From<&RxEntry> for RxContainerItem {
    fn from(value: &RxEntry) -> Self {
        Self::Entry(value.uuid)
    }
}

impl From<&RxGroup> for RxContainerItem {
    fn from(value: &RxGroup) -> Self {
        let mut children = vec![];
        children.append(&mut value.subgroups.clone());
        children.append(&mut value.entries.clone());

        Self::Grouping(RxContainerGrouping {
            uuid: value.uuid,
            children: children,
            grouping: RxGroupingType::Group,
        })
    }
}

impl From<&RxTemplate> for RxContainerItem {
    fn from(value: &RxTemplate) -> Self {
        Self::Grouping(RxContainerGrouping {
            uuid: value.uuid,
            children: value.entry_uuids.clone(),
            grouping: RxGroupingType::Template,
        })
    }
}

#[derive(Clone, Copy)]
pub enum RxGroupingType {
    Template,
    Group,
    Root,
}

#[derive(Clone)]
pub struct RxContainerGrouping {
    uuid: Uuid,
    children: Vec<Uuid>,
    grouping: RxGroupingType,
}

#[allow(dead_code)]
impl RxContainerGrouping {
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn grouping_type(&self) -> RxGroupingType {
        self.grouping
    }
}

#[derive(Clone)]
pub struct RxContainerWithDb<'db>(&'db RxContainer, &'db RxDatabase);

impl<'db> RxContainerWithDb<'db> {
    pub fn new(container: &'db RxContainer, db: &'db RxDatabase) -> Self {
        Self(container, db)
    }

    pub fn container(&self) -> &RxContainer {
        &self.0
    }

    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn uuid(&self) -> Uuid {
        self.container().uuid()
    }

    pub fn find_children(&self, search_term: Option<&str>) -> Vec<RxContainer> {
        let container = self.container();
        let item = container.item();

        match item {
            &RxContainerItem::Grouping(ref grouping) => match grouping {
                RxContainerGrouping { uuid, grouping, .. }
                    if matches!(grouping, RxGroupingType::Template) =>
                {
                    if container.is_root() {
                        self.db()
                            .find_templates(search_term)
                            .map(|tmplt| RxContainer::from(tmplt, false))
                            .collect()
                    } else {
                        self.db()
                            .get_template(*uuid)
                            .map(|template| {
                                self.1
                                    .entries_iter_by_uuid(
                                        template.entry_uuids.as_slice(),
                                        search_term,
                                    )
                                    .map(|ent| RxContainer::from(ent, false))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    }
                }
                RxContainerGrouping { uuid, grouping, .. }
                    if matches!(grouping, RxGroupingType::Group) =>
                {
                    let subgroups_iter = self.1.filter_subgroups(*uuid, search_term);
                    let entries = self.1.get_entries(*uuid, search_term);

                    // Groups first, then entries below.
                    let mut item_list: Vec<_> = subgroups_iter
                        .map(|subgroup| RxContainer::from(subgroup, false))
                        .collect();

                    item_list.append(
                        &mut entries
                            .into_iter()
                            .map(|ent| RxContainer::from(ent, false))
                            .collect(),
                    );

                    item_list
                }
                _ => vec![],
            },
            _ => vec![],
        }
    }
}

pub struct RxRootWithDb<'root, 'db>(&'root RxRoot, &'db RxDatabase)
where
    'root: 'db;

impl<'root, 'db> RxRootWithDb<'root, 'db> {
    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn root(&self) -> RxContainerWithDb<'_> {
        RxContainerWithDb(&self.0.root, self.db())
    }

    pub fn children(&self) -> Vec<RxContainerWithDb<'_>> {
        self.0
            .children()
            .into_iter()
            .map(|container| RxContainerWithDb(container, self.db()))
            .collect()
    }
}

// All of this will work, exept that we can never memoize or store it,
// because it's all built on references. So the lifetime of one of
// these is tied to the duration of a borrow of the DB. The proper
// solution would be to use owned data, which sort of defeats the
// purpose? But then again, it boils down to borrowing the
// template/group/entry. So if we remove the refs on template/group
// and just make the grouping a generic struct with a name, and then
// store only UUIDs for entries, we can construct a hierarchy separate
// from the main DB, and then keep RxRootWithDb functioning to
// actually load entries.
pub trait VirtualHierarchy {
    fn create(self) -> RxRoot;
}

#[derive(Clone, Copy)]
pub struct AllTemplates<'db>(pub &'db RxDatabase);

impl VirtualHierarchy for AllTemplates<'_> {
    fn create(self) -> RxRoot {
        let mut root = RxRoot::virtual_root();

        root.children = self
            .0
            .templates_iter()
            .map(|t| RxContainer::from(t, false))
            .collect();

        root
    }
}

// So now that we have converted this to owned data, we should be able
// to store current RxRoot in actor. Then when we want to do stuff
// with it, we can call with_db to get a usable ref thingy. Might need
// to correct lifetimes. TODO fix the rxpagetype stuff.
