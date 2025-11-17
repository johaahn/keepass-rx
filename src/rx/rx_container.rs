/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use indexmap::{IndexMap, IndexSet};
use keepass::db::Group;
use std::{collections::HashMap, mem};
use unicase::UniCase;
use uuid::Uuid;
use zeroize::Zeroize;

use super::{RxDatabase, RxEntry, RxGroup, RxTemplate, RxValue, icons::RxIcon};

fn search_contained_ref(contained_ref: &RxContainedRef, term: &str) -> bool {
    match contained_ref {
        RxContainedRef::Entry(entry) => search_entry(entry, term),
        RxContainedRef::Group(group) => search_group(group, term),
        RxContainedRef::Template(template) => search_template(template, term),
    }
}

fn search_group(group: &RxGroup, term: &str) -> bool {
    UniCase::new(&group.name).to_folded_case().contains(term)
}

fn search_template(template: &RxTemplate, term: &str) -> bool {
    UniCase::new(&template.name).to_folded_case().contains(term)
}

fn search_entry(entry: &RxEntry, term: &str) -> bool {
    let username = entry.username().and_then(|u| {
        u.value()
            .map(|secret| UniCase::new(secret).to_folded_case())
    });

    let url = entry.url().and_then(|u| {
        u.value()
            .map(|secret| UniCase::new(secret).to_folded_case())
    });

    let title = entry.title().and_then(|u| {
        u.value()
            .map(|secret| UniCase::new(secret).to_folded_case())
    });

    let contains_username = username.map(|u| u.contains(term)).unwrap_or(false);
    let contains_url = url.map(|u| u.contains(term)).unwrap_or(false);
    let contains_title = title.map(|t| t.contains(term)).unwrap_or(false);

    contains_username || contains_url || contains_title
}

#[derive(Clone)]
pub struct RxRoot {
    pub(super) root_container: RxContainer,
    pub(super) all_containers: IndexSet<Uuid>,
}

impl RxRoot {
    pub fn uuid(&self) -> Uuid {
        self.root_container.uuid()
    }

    pub fn virtual_root(children: Vec<RxContainer>) -> Self {
        let all_child_uuids: IndexSet<Uuid> = children
            .iter()
            .flat_map(|child| child.child_uuids_recursive())
            .collect();

        Self {
            all_containers: all_child_uuids,
            root_container: RxContainer {
                item: RxContainerItem::VirtualRoot(children),
                is_root: true,
                contained_type: RxContainedType::VirtualRoot,
            },
        }
    }

    pub fn with_db<'root, 'db>(
        &'root self,
        db: &'db RxDatabase,
    ) -> RxContainerWithDb<'root, 'db> {
        RxContainerWithDb::new(&self.root_container, db)
    }

    pub fn get_container(&self, container_uuid: Uuid) -> Option<&RxContainer> {
        self.root_container.get_container_recursive(container_uuid)
    }
}

pub trait IntoContainer {
    fn into_container(&self, db: &RxDatabase) -> RxContainer;
}

/// What exactly this container points to. The actual resource, as
/// opposed to its function (e.g. a Grouping can be a Group or a
/// Template).
#[derive(Clone, Copy)]
pub enum RxContainedType {
    Entry,
    Group,
    Template,
    VirtualRoot,
}

#[derive(Clone)]
pub struct RxContainer {
    item: RxContainerItem,
    is_root: bool,
    contained_type: RxContainedType,
}

impl RxContainer {
    pub fn from<T>(item: T, db: &RxDatabase) -> Self
    where
        T: IntoContainer,
    {
        item.into_container(db)
    }

    pub fn with_db<'cnt, 'db>(
        &'cnt self,
        db: &'db RxDatabase,
    ) -> RxContainerWithDb<'cnt, 'db> {
        RxContainerWithDb(self, db)
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

    pub fn child_groupings(&self) -> Vec<&RxContainer> {
        self.children()
            .iter()
            .filter_map(|child| match child.item {
                RxContainerItem::Grouping(_) => Some(child),
                _ => None,
            })
            .collect()
    }

    pub fn child_uuids_recursive(&self) -> IndexSet<Uuid> {
        let mut these_uuids = IndexSet::new();

        for child in self.children() {
            these_uuids.insert(child.uuid());

            for child_uuid in child.child_uuids_recursive() {
                these_uuids.insert(child_uuid);
            }
        }

        these_uuids
    }

    pub fn child_containers_recursive(&self) -> IndexMap<Uuid, &RxContainer> {
        let mut children = IndexMap::new();

        for child in self.children() {
            children.insert(child.uuid(), child);
            children.append(&mut child.child_containers_recursive());
        }

        children
    }

    /// Recursively fetch a container by UUID somewhere in this tree
    /// (including this container itself).
    pub fn get_container_recursive(&self, uuid: Uuid) -> Option<&RxContainer> {
        if self.uuid() == uuid {
            Some(self)
        } else {
            self.child_containers_recursive().get(&uuid).map(|v| &**v)
        }
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
            .flat_map(|id| db.get_group(id).map(|group| RxContainer::from(group, db)))
            .collect();

        RxContainer {
            is_root: db.root_group().uuid == self.uuid,
            contained_type: RxContainedType::Group,
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
            .flat_map(|id| db.get_entry(*id).map(|ent| RxContainer::from(ent, db)))
            .collect();

        RxContainer {
            is_root: false,
            contained_type: RxContainedType::Template,
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
            contained_type: RxContainedType::Entry,
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
pub struct RxContainerWithDb<'cnt, 'db>(&'cnt RxContainer, &'db RxDatabase);

impl<'cnt, 'db> RxContainerWithDb<'cnt, 'db> {
    pub fn new(container: &'cnt RxContainer, db: &'db RxDatabase) -> Self {
        Self(container, db)
    }

    pub fn container(&self) -> &RxContainer {
        &self.0
    }

    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn get_ref(&self) -> Option<RxContainedRef<'db>> {
        match self.container().contained_type {
            RxContainedType::Group => self
                .db()
                .get_group(self.uuid())
                .map(|g| RxContainedRef::Group(g)),
            RxContainedType::Template => self
                .db()
                .get_template(self.uuid())
                .map(|t| RxContainedRef::Template(t)),
            RxContainedType::Entry => self
                .db()
                .get_entry(self.uuid())
                .map(|e| RxContainedRef::Entry(e)),
            _ => None,
        }
    }

    pub fn uuid(&self) -> Uuid {
        self.container().uuid()
    }

    pub fn search_children_immediate(
        &'cnt self,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'db>> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let containers_in_tree = self.container().children();

        containers_in_tree
            .into_iter()
            .map(|child| {
                let maybe_contained_ref = child.with_db(self.db()).get_ref();
                maybe_contained_ref.and_then(|contained_ref| {
                    if let Some(term) = &search_term {
                        match search_contained_ref(&contained_ref, term) {
                            true => Some(contained_ref),
                            false => None,
                        }
                    } else {
                        Some(contained_ref)
                    }
                })
            })
            .flatten()
            .collect::<Vec<_>>()
    }

    pub fn child_groupings_immedate(&self) -> Vec<RxContainedRef<'db>> {
        self.container()
            .child_groupings()
            .into_iter()
            .flat_map(|child| child.with_db(self.db()).get_ref())
            .collect()
    }

    pub fn search_children_recursive(
        &'cnt self,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'db>> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let container = self.container().get_container_recursive(container_uuid);

        let containers_in_tree = container
            .map(|container| container.child_containers_recursive())
            .map(|child_containers| child_containers.into_iter().map(|(_, child)| child));

        let filtered_by_search = containers_in_tree.map(|containers_iter| {
            containers_iter
                .filter_map(|container| {
                    let maybe_contained_ref = container.with_db(self.db()).get_ref();
                    maybe_contained_ref.and_then(|contained_ref| {
                        if let Some(term) = &search_term {
                            match search_contained_ref(&contained_ref, term) {
                                true => Some(contained_ref),
                                false => None,
                            }
                        } else {
                            Some(contained_ref)
                        }
                    })
                })
                .collect::<Vec<_>>()
        });

        filtered_by_search.unwrap_or_default()
    }
}

/// A reference to the actual thing in the database, as pointed to by the container.
#[derive(Clone, Copy)]
pub enum RxContainedRef<'db> {
    Entry(&'db RxEntry),
    Group(&'db RxGroup),
    Template(&'db RxTemplate),
}

impl RxContainedRef<'_> {
    pub fn uuid(&self) -> Uuid {
        match self {
            RxContainedRef::Entry(entry) => entry.uuid,
            RxContainedRef::Group(group) => group.uuid,
            RxContainedRef::Template(template) => template.uuid,
        }
    }

    pub fn name(&self) -> String {
        match self {
            RxContainedRef::Entry(entry) => entry
                .title()
                .and_then(|t| t.value().as_deref().cloned())
                .unwrap_or_else(|| "Untitled".to_string()),
            RxContainedRef::Group(group) => group.name.clone(),
            RxContainedRef::Template(template) => template.name.clone(),
        }
    }

    pub fn parent(&self) -> Option<Uuid> {
        match self {
            RxContainedRef::Entry(entry) => Some(entry.parent_group),
            RxContainedRef::Group(group) => group.parent,
            // Templates never have a parent UUID, due to virtual root.
            RxContainedRef::Template(_) => None,
        }
    }
}
