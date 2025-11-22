/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use indexmap::{IndexMap, IndexSet};
use std::{cmp::Ordering, rc::Rc};
use unicase::UniCase;
use uuid::Uuid;

use super::{RxDatabase, RxEntry, RxGroup, RxTag, RxTemplate};

fn search_contained_ref(contained_ref: &RxContainedRef, term: &str) -> bool {
    match contained_ref {
        RxContainedRef::Entry(entry) => search_entry(entry, term),
        RxContainedRef::Group(group) => search_group(group, term),
        RxContainedRef::Template(template) => search_template(template, term),
        RxContainedRef::Tag(tag) => search_tag(tag, term),
        RxContainedRef::VirtualRoot(_) => true,
    }
}

fn search_tag(tag: &RxTag, term: &str) -> bool {
    UniCase::new(&tag.name).to_folded_case().contains(term)
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
#[allow(dead_code)]
pub struct RxRoot {
    pub(super) root_container: Rc<RxContainer>,
    pub(super) all_containers: IndexSet<Uuid>,
}

#[allow(dead_code)]
impl RxRoot {
    pub fn uuid(&self) -> Uuid {
        self.root_container.uuid()
    }

    pub fn root_name(&self) -> String {
        self.root_container
            .get_ref()
            .map(|r| r.name())
            .unwrap_or_else(|| "No Name".to_string())
    }

    pub fn virtual_root(name: &str, children: Vec<RxContainer>) -> Self {
        let all_child_uuids: IndexSet<Uuid> = children
            .iter()
            .flat_map(|child| child.child_uuids_recursive())
            .collect();

        Self {
            all_containers: all_child_uuids,
            root_container: Rc::new(RxContainer {
                item: RxContainerItem::VirtualRoot(name.to_owned(), children),
                is_root: true,
                contained_type: RxContainedType::VirtualRoot,
            }),
        }
    }

    /// Find a container somewhere in this root.
    pub fn get_container(&self, container_uuid: Uuid) -> Option<&RxContainer> {
        self.root_container.get_container_recursive(container_uuid)
    }
}

pub trait IntoContainer {
    fn into_container(&self, db: &RxDatabase) -> RxContainer;
}

/// What exactly this container points to. The actual resource, as
/// opposed to its function (e.g. a Grouping can be a Group or a
/// Template or a Tag).
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum RxContainedType {
    Entry,
    Group,
    Template,
    Tag,
    VirtualRoot,
}

#[derive(Clone)]
pub struct RxContainer {
    item: RxContainerItem,
    is_root: bool,
    contained_type: RxContainedType,
}

#[allow(dead_code)]
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

    pub fn get_ref(&self) -> Option<RxContainedRef> {
        match self.contained_type {
            RxContainedType::Group | RxContainedType::Template | RxContainedType::Tag => {
                self.item().grouping().and_then(|g| g.contained_ref())
            }
            RxContainedType::Entry => self.item().entry().map(|e| RxContainedRef::Entry(e)),
            RxContainedType::VirtualRoot => match self.item() {
                RxContainerItem::VirtualRoot(name, _) => {
                    Some(RxContainedRef::VirtualRoot(name.clone()))
                }
                _ => None,
            },
        }
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

    pub fn search_children_immediate(&self, search_term: Option<&str>) -> Vec<RxContainedRef> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let immediate_children = self.children();

        immediate_children
            .into_iter()
            .map(|child| {
                child.get_ref().and_then(|contained_ref| {
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

    pub fn child_groupings_immedate(&self) -> Vec<RxContainedRef> {
        self.child_groupings()
            .into_iter()
            .flat_map(|child| child.get_ref())
            .collect()
    }

    pub fn search_children_recursive(
        &self,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let container = self.get_container_recursive(container_uuid);

        let containers_in_tree = container
            .map(|container| container.child_containers_recursive())
            .map(|child_containers| child_containers.into_iter().map(|(_, child)| child));

        let filtered_by_search = containers_in_tree.map(|containers_iter| {
            containers_iter
                .filter_map(|container| {
                    container.get_ref().and_then(|contained_ref| {
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

#[derive(Clone)]
pub enum RxContainerItem {
    /// VirtualRoot is for containers that don't have a clear actual
    /// existing root, e.g. list of all templates has no parent that
    /// could be root.
    VirtualRoot(String, Vec<RxContainer>), // name + children
    Grouping(RxContainerGrouping),
    Entry(Rc<RxEntry>),
}

impl RxContainerItem {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Grouping(grouping) => grouping.uuid(),
            Self::Entry(entry) => entry.uuid,
            Self::VirtualRoot(_, _) => Uuid::default(),
        }
    }

    pub fn grouping(&self) -> Option<&RxGrouping> {
        match self {
            Self::Grouping(grouping) => Some(grouping.grouping()),
            Self::Entry(_) => None,
            Self::VirtualRoot(_, _) => Some(&RxGrouping::VirtualRoot),
        }
    }

    pub fn entry(&self) -> Option<Rc<RxEntry>> {
        match self {
            Self::Entry(entry) => Some(entry.clone()),
            _ => None,
        }
    }

    pub fn children(&self) -> &[RxContainer] {
        match self {
            Self::Grouping(grouping) => grouping.children.as_slice(),
            Self::Entry(_) => &[],
            Self::VirtualRoot(_, children) => children.as_slice(),
        }
    }
}

impl From<Rc<RxEntry>> for RxContainerItem {
    fn from(value: Rc<RxEntry>) -> Self {
        Self::Entry(value.clone())
    }
}

impl IntoContainer for RxTag {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let entries: Vec<_> = self
            .entry_uuids
            .as_slice()
            .into_iter()
            .flat_map(|id| db.get_entry(*id).map(|entry| RxContainer::from(entry, db)))
            .collect();

        RxContainer {
            is_root: false,
            contained_type: RxContainedType::Tag,
            item: RxContainerItem::Grouping(RxContainerGrouping {
                children: entries,
                grouping: RxGrouping::Tag(self.clone()),
            }),
        }
    }
}

impl IntoContainer for Rc<RxGroup> {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let mut subgroups: Vec<_> = self
            .subgroups
            .as_slice()
            .into_iter()
            .flat_map(|id| db.get_group(*id).map(|group| RxContainer::from(group, db)))
            .collect();

        let mut entries: Vec<_> = self
            .entries
            .as_slice()
            .into_iter()
            .flat_map(|id| db.get_entry(*id).map(|entry| RxContainer::from(entry, db)))
            .collect();

        let mut children = vec![];
        children.append(&mut subgroups);
        children.append(&mut entries);

        RxContainer {
            is_root: db.root_group().uuid == self.uuid,
            contained_type: RxContainedType::Group,
            item: RxContainerItem::Grouping(RxContainerGrouping {
                children: children,
                grouping: RxGrouping::Group(self.clone()),
            }),
        }
    }
}

impl IntoContainer for Rc<RxTemplate> {
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
                children: children,
                grouping: RxGrouping::Template(self.clone()),
            }),
        }
    }
}

impl IntoContainer for Rc<RxEntry> {
    fn into_container(&self, _: &RxDatabase) -> RxContainer {
        RxContainer {
            contained_type: RxContainedType::Entry,
            is_root: false,
            item: RxContainerItem::Entry(self.clone()),
        }
    }
}

#[derive(Clone)]
pub enum RxGrouping {
    Template(Rc<RxTemplate>),
    Tag(RxTag),
    Group(Rc<RxGroup>),
    VirtualRoot,
}

impl RxGrouping {
    pub fn contained_ref(&self) -> Option<RxContainedRef> {
        match &self {
            RxGrouping::Group(group) => Some(RxContainedRef::Group(group.clone())),
            RxGrouping::Template(template) => Some(RxContainedRef::Template(template.clone())),
            RxGrouping::Tag(tag) => Some(RxContainedRef::Tag(tag.clone())),
            RxGrouping::VirtualRoot => None,
        }
    }
}

#[derive(Clone)]
pub struct RxContainerGrouping {
    children: Vec<RxContainer>,
    grouping: RxGrouping,
}

#[allow(dead_code)]
impl RxContainerGrouping {
    pub fn uuid(&self) -> Uuid {
        match &self.grouping {
            RxGrouping::Group(group) => group.uuid,
            RxGrouping::Template(template) => template.uuid,
            RxGrouping::Tag(tag) => tag.uuid,
            RxGrouping::VirtualRoot => Uuid::default(),
        }
    }

    pub fn grouping(&self) -> &RxGrouping {
        &self.grouping
    }
}

/// A reference to the actual thing in the database, as pointed to by the container.
#[derive(Clone)]
pub enum RxContainedRef {
    VirtualRoot(String), // name
    Group(Rc<RxGroup>),
    Template(Rc<RxTemplate>),
    Tag(RxTag),
    Entry(Rc<RxEntry>),
}

impl PartialEq for RxContainedRef {
    fn eq(&self, other: &Self) -> bool {
        self.variant_rank() == other.variant_rank()
    }
}

impl Eq for RxContainedRef {}

impl PartialOrd for RxContainedRef {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.variant_rank().cmp(&other.variant_rank()))
    }
}

impl Ord for RxContainedRef {
    fn cmp(&self, other: &Self) -> Ordering {
        self.variant_rank().cmp(&other.variant_rank())
    }
}

#[allow(dead_code)]
impl RxContainedRef {
    fn variant_rank(&self) -> u8 {
        match self {
            RxContainedRef::VirtualRoot(_) => 0,
            RxContainedRef::Group(_) => 1,
            RxContainedRef::Template(_) => 2,
            RxContainedRef::Tag(_) => 3,
            RxContainedRef::Entry(_) => 4,
        }
    }

    pub fn uuid(&self) -> Uuid {
        match self {
            RxContainedRef::Entry(entry) => entry.uuid,
            RxContainedRef::Group(group) => group.uuid,
            RxContainedRef::Template(template) => template.uuid,
            RxContainedRef::Tag(tag) => tag.uuid,
            RxContainedRef::VirtualRoot(_) => Uuid::default(),
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
            RxContainedRef::Tag(tag) => tag.name.clone(),
            RxContainedRef::VirtualRoot(name) => name.clone(),
        }
    }

    pub fn parent(&self) -> Option<Uuid> {
        match self {
            RxContainedRef::Entry(entry) => Some(entry.parent_group),
            RxContainedRef::Group(group) => group.parent,
            RxContainedRef::Template(_) | RxContainedRef::Tag(_) => Some(Uuid::default()), //virtual root
            RxContainedRef::VirtualRoot(_) => None,
        }
    }
}
