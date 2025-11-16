use keepass::db::Group;
use std::{collections::HashMap, mem};
use uuid::Uuid;
use zeroize::Zeroize;

/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use super::{RxDatabase, RxEntry, RxGroup, RxTemplate, RxValue, icons::RxIcon};

pub struct RxRoot(RxContainer);

impl RxRoot {
    pub fn virtual_root(children: Vec<RxContainer>) -> Self {
        Self(RxContainer {
            item: RxContainerItem::VirtualRoot(children),
            is_root: true,
        })
    }

    pub fn with_db<'root, 'db>(&'root self, db: &'db RxDatabase) -> RxRootWithDb<'root, 'db> {
        RxRootWithDb(self, db)
    }
}

pub trait IntoContainer {
    fn into_container(&self, db: &RxDatabase) -> RxContainer;
}

#[derive(Clone)]
pub struct RxContainer {
    item: RxContainerItem,
    is_root: bool,
}

impl RxContainer {
    pub fn from<T>(item: T, db: &RxDatabase) -> Self
    where
        T: IntoContainer,
    {
        item.into_container(db)
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

    pub fn children(&self) -> &[RxContainer] {
        self.item.children()
    }
}

#[derive(Clone)]
pub enum RxContainerItem {
    /// VirtualRoot is for containers that don't have a clear actual
    /// existing root, e.g. list of all templates has no parent that
    /// could be root.
    VirtualRoot(Vec<RxContainer>),
    Grouping(RxContainerGrouping),
    Entry(Uuid),
}

impl RxContainerItem {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Grouping(grouping) => grouping.uuid(),
            Self::Entry(entry_uuid) => *entry_uuid,
            Self::VirtualRoot(_) => Uuid::default(),
        }
    }

    pub fn grouping_type(&self) -> Option<RxGroupingType> {
        match self {
            Self::Grouping(grouping) => Some(grouping.grouping_type()),
            Self::Entry(_) => None,
            Self::VirtualRoot(_) => Some(RxGroupingType::Root),
        }
    }

    pub fn children(&self) -> &[RxContainer] {
        match self {
            Self::Grouping(grouping) => grouping.children.as_slice(),
            Self::Entry(_) => &[],
            Self::VirtualRoot(children) => children.as_slice(),
        }
    }
}

impl From<&RxEntry> for RxContainerItem {
    fn from(value: &RxEntry) -> Self {
        Self::Entry(value.uuid)
    }
}

impl IntoContainer for &RxGroup {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let children: Vec<_> = [self.subgroups.as_slice(), self.entries.as_slice()]
            .concat()
            .into_iter()
            .flat_map(|id| db.get_container(id))
            .collect();

        RxContainer {
            is_root: db.root_group().uuid == self.uuid,
            item: RxContainerItem::Grouping(RxContainerGrouping {
                uuid: self.uuid,
                children: children,
                grouping: RxGroupingType::Group,
            }),
        }
    }
}

impl IntoContainer for &RxTemplate {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let children: Vec<_> = self
            .entry_uuids
            .iter()
            .flat_map(|id| db.get_container(*id))
            .collect();

        RxContainer {
            is_root: false,
            item: RxContainerItem::Grouping(RxContainerGrouping {
                uuid: self.uuid,
                children: children,
                grouping: RxGroupingType::Template,
            }),
        }
    }
}

impl IntoContainer for &RxEntry {
    fn into_container(&self, _: &RxDatabase) -> RxContainer {
        RxContainer {
            is_root: false,
            item: RxContainerItem::Entry(self.uuid),
        }
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
    children: Vec<RxContainer>,
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
                            .map(|tmplt| RxContainer::from(tmplt, self.db()))
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
                                    .map(|ent| RxContainer::from(ent, self.db()))
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
                        .map(|subgroup| RxContainer::from(subgroup, self.db()))
                        .collect();

                    item_list.append(
                        &mut entries
                            .into_iter()
                            .map(|ent| RxContainer::from(ent, self.db()))
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
    pub fn new(root: &'root RxRoot, db: &'db RxDatabase) -> Self {
        Self(root, db)
    }

    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn root(&self) -> RxContainerWithDb<'_> {
        RxContainerWithDb(&self.0.0, self.db())
    }

    pub fn children(&self) -> Vec<RxContainerWithDb<'_>> {
        let containers: Vec<_> = self.0.0.children().into_iter().collect();

        containers
            .into_iter()
            .map(|container| RxContainerWithDb(container, self.db()))
            .collect()
    }
}

/// An arbitrary hierarchical view into the password database.
pub trait VirtualHierarchy {
    fn create(self) -> RxRoot;
}

#[derive(Clone, Copy)]
pub struct AllTemplates<'db>(pub &'db RxDatabase);

impl VirtualHierarchy for AllTemplates<'_> {
    fn create(self) -> RxRoot {
        let children: Vec<_> = self
            .0
            .templates_iter()
            .map(|t| RxContainer::from(t, &self.0))
            .collect();

        RxRoot::virtual_root(children)
    }
}

pub struct DefaultView<'db>(pub &'db RxDatabase);

impl VirtualHierarchy for DefaultView<'_> {
    fn create(self) -> RxRoot {
        RxRoot(RxContainer::from(self.0.root_group(), self.0))
    }
}
