/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use indexmap::{IndexMap, IndexSet};
use std::{borrow::Cow, cmp::Ordering, rc::Rc};
use unicase::UniCase;
use uuid::Uuid;

use super::{
    RxDatabase, RxEntry, RxGroup, RxSavedSearch, RxSearchType, RxTag, RxTemplate,
    search::search_contained_ref,
};

fn contained_sort_name(item: &RxContainedRef<'_>) -> String {
    UniCase::new(item.name().into_owned()).to_folded_case()
}

fn compare_contained_refs(this: &RxContainedRef<'_>, that: &RxContainedRef<'_>) -> Ordering {
    this.variant_rank()
        .cmp(&that.variant_rank())
        .then_with(|| contained_sort_name(this).cmp(&contained_sort_name(that)))
        .then_with(|| this.uuid().cmp(&that.uuid()))
}

pub(crate) fn sort_contained_refs(items: &mut [RxContainedRef<'_>]) {
    items.sort_by(compare_contained_refs);
}

pub(crate) fn sort_containers(items: &mut [RxContainer]) {
    items.sort_by(|this, that| match (this.contained_ref(), that.contained_ref()) {
        (Some(this), Some(that)) => compare_contained_refs(&this, &that),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => this.uuid().cmp(&that.uuid()),
    });
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
            .contained_ref()
            .map(|r| r.name().into_owned())
            .unwrap_or_else(|| "No Name".to_string())
    }

    pub fn virtual_root(name: &str, children: Vec<RxContainer>) -> Self {
        let mut children = children;
        sort_containers(&mut children);

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

impl<T> IntoContainer for &T
where
    T: IntoContainer,
{
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        (*self).into_container(db)
    }
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
    SavedSearch,
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

    pub fn contained_ref(&self) -> Option<RxContainedRef<'_>> {
        match self.contained_type {
            RxContainedType::Group
            | RxContainedType::Template
            | RxContainedType::Tag
            | RxContainedType::SavedSearch => {
                self.item().grouping().and_then(|g| g.contained_ref())
            }
            RxContainedType::Entry => self
                .item()
                .entry_ref()
                .map(|entry| RxContainedRef::Entry(Cow::Borrowed(entry))),
            RxContainedType::VirtualRoot => match self.item() {
                RxContainerItem::VirtualRoot(name, _) => {
                    Some(RxContainedRef::VirtualRoot(Cow::Borrowed(name.as_str())))
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

    pub fn search_children_immediate(
        &self,
        search_type: RxSearchType,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'_>> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let mut results = self
            .children()
            .iter()
            .filter_map(|child| {
                let contained_ref = child.contained_ref()?;
                let is_match = search_term.as_ref().is_none_or(|term| {
                    search_contained_ref(&contained_ref, search_type, term)
                });
                is_match.then_some(contained_ref)
            })
            .collect::<Vec<_>>();

        sort_contained_refs(&mut results);
        results
    }

    pub fn child_groupings_immedate(&self) -> Vec<RxContainedRef<'_>> {
        self.child_groupings()
            .into_iter()
            .filter_map(|child| child.contained_ref())
            .collect()
    }

    pub fn search_children_recursive(
        &self,
        search_type: RxSearchType,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'_>> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let container = self.get_container_recursive(container_uuid);

        let containers_in_tree = container
            .map(|container| container.child_containers_recursive())
            .map(|child_containers| child_containers.into_iter().map(|(_, child)| child));

        let filtered_by_search = containers_in_tree.map(|containers_iter| {
            containers_iter
                .filter_map(|container| {
                    let contained_ref = container.contained_ref()?;
                    let is_match = search_term.as_ref().is_none_or(|term| {
                        search_contained_ref(&contained_ref, search_type, term)
                    });

                    is_match.then_some(contained_ref)
                })
                .collect::<Vec<_>>()
        });

        let mut results = filtered_by_search.unwrap_or_default();
        sort_contained_refs(&mut results);
        results
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

    pub fn entry_ref(&self) -> Option<&Rc<RxEntry>> {
        match self {
            Self::Entry(entry) => Some(entry),
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
        Self::Entry(value)
    }
}

impl IntoContainer for RxTag {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let mut entries: Vec<_> = self
            .entry_uuids
            .as_slice()
            .into_iter()
            .flat_map(|id| db.get_entry(*id).map(|entry| RxContainer::from(entry, db)))
            .collect();
        sort_containers(&mut entries);

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

impl IntoContainer for RxSavedSearch {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let mut entries: Vec<_> = self
            .entry_uuids
            .as_slice()
            .into_iter()
            .flat_map(|id| db.get_entry(*id).map(|entry| RxContainer::from(entry, db)))
            .collect();
        sort_containers(&mut entries);

        RxContainer {
            is_root: false,
            contained_type: RxContainedType::SavedSearch,
            item: RxContainerItem::Grouping(RxContainerGrouping {
                children: entries,
                grouping: RxGrouping::SavedSearch(self.clone()),
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
        sort_containers(&mut subgroups);
        sort_containers(&mut entries);

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
        let mut children: Vec<_> = self
            .entry_uuids
            .iter()
            .flat_map(|id| db.get_entry(*id).map(|ent| RxContainer::from(ent, db)))
            .collect();
        sort_containers(&mut children);

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
    SavedSearch(RxSavedSearch),
    Group(Rc<RxGroup>),
    VirtualRoot,
}

impl RxGrouping {
    pub fn contained_ref(&self) -> Option<RxContainedRef<'_>> {
        match &self {
            RxGrouping::Group(group) => Some(RxContainedRef::Group(Cow::Borrowed(group))),
            RxGrouping::Template(template) => {
                Some(RxContainedRef::Template(Cow::Borrowed(template)))
            }
            RxGrouping::Tag(tag) => Some(RxContainedRef::Tag(Cow::Borrowed(tag))),
            RxGrouping::SavedSearch(search) => {
                Some(RxContainedRef::SavedSearch(Cow::Borrowed(search)))
            }
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
            RxGrouping::SavedSearch(search) => search.uuid,
            RxGrouping::VirtualRoot => Uuid::default(),
        }
    }

    pub fn grouping(&self) -> &RxGrouping {
        &self.grouping
    }
}

/// A reference to the actual thing in the database, as pointed to by the container.
#[derive(Clone)]
pub enum RxContainedRef<'a> {
    VirtualRoot(Cow<'a, str>), // name
    Group(Cow<'a, Rc<RxGroup>>),
    Template(Cow<'a, Rc<RxTemplate>>),
    Tag(Cow<'a, RxTag>),
    SavedSearch(Cow<'a, RxSavedSearch>),
    Entry(Cow<'a, Rc<RxEntry>>),
}

impl PartialEq for RxContainedRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        compare_contained_refs(self, other) == Ordering::Equal
    }
}

impl Eq for RxContainedRef<'_> {}

impl PartialOrd for RxContainedRef<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(compare_contained_refs(self, other))
    }
}

impl Ord for RxContainedRef<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_contained_refs(self, other)
    }
}

#[allow(dead_code)]
impl RxContainedRef<'_> {
    fn variant_rank(&self) -> u8 {
        match self {
            RxContainedRef::VirtualRoot(_) => 0,
            RxContainedRef::Group(_) => 1,
            RxContainedRef::Template(_) => 2,
            RxContainedRef::Tag(_) => 3,
            RxContainedRef::SavedSearch(_) => 4,
            RxContainedRef::Entry(_) => 5,
        }
    }

    pub fn uuid(&self) -> Uuid {
        match self {
            RxContainedRef::Entry(entry) => entry.as_ref().uuid,
            RxContainedRef::Group(group) => group.as_ref().uuid,
            RxContainedRef::Template(template) => template.as_ref().uuid,
            RxContainedRef::Tag(tag) => tag.as_ref().uuid,
            RxContainedRef::SavedSearch(search) => search.as_ref().uuid,
            RxContainedRef::VirtualRoot(_) => Uuid::default(),
        }
    }

    pub fn name(&self) -> Cow<'_, str> {
        match self {
            RxContainedRef::Entry(entry) => entry
                .as_ref()
                .title()
                .and_then(|t| t.value())
                .map(|mut title| Cow::Owned(std::mem::take(&mut *title)))
                .unwrap_or_else(|| Cow::Borrowed("Untitled")),
            RxContainedRef::Group(group) => Cow::Borrowed(group.as_ref().name.as_str()),
            RxContainedRef::Template(template) => {
                Cow::Borrowed(template.as_ref().name.as_str())
            }
            RxContainedRef::Tag(tag) => Cow::Borrowed(tag.as_ref().name.as_str()),
            RxContainedRef::SavedSearch(search) => {
                Cow::Borrowed(search.as_ref().name.as_str())
            }
            RxContainedRef::VirtualRoot(name) => Cow::Borrowed(name.as_ref()),
        }
    }

    pub fn parent(&self) -> Option<Uuid> {
        match self {
            RxContainedRef::Entry(entry) => Some(entry.as_ref().parent_group),
            RxContainedRef::Group(group) => group.as_ref().parent,
            RxContainedRef::Template(_)
            | RxContainedRef::Tag(_)
            | RxContainedRef::SavedSearch(_) => Some(Uuid::default()), //virtual root
            RxContainedRef::VirtualRoot(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use keepass::db::{Database as KeePassDatabase, fields};
    use keyring::set_default_credential_builder;
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use crate::rx::{ZeroableDatabase, virtual_hierarchy::{DefaultView, VirtualHierarchy}};

    use super::*;

    fn title_for(item: &RxContainedRef<'_>) -> String {
        item.name().into_owned()
    }

    #[test]
    fn default_view_orders_groups_before_entries_and_names_case_insensitively() {
        set_default_credential_builder(keyring::mock::default_credential_builder());

        let mut db = KeePassDatabase::new();

        {
            let mut root = db.root_mut();

            root.add_entry().edit(|entry| {
                entry.set_unprotected(fields::TITLE, "zebra");
            });
            root.add_group().edit(|group| {
                group.name = "bravo".into();
            });
            root.add_entry().edit(|entry| {
                entry.set_unprotected(fields::TITLE, "Alpha".to_string());
            });
            root.add_group().edit(|group| {
                group.name = "charlie".into();
            });
        }

        let rx_db = RxDatabase::new(Zeroizing::new(ZeroableDatabase(db))).expect("load rx db");
        let view = DefaultView::new(&rx_db);
        let results = view.search(RxSearchType::CaseInsensitive, view.root().uuid(), None);
        let names: Vec<_> = results.iter().map(title_for).collect();

        assert_eq!(names, vec!["bravo", "charlie", "Alpha", "zebra"]);
    }

    #[test]
    fn default_view_uses_uuid_tiebreaker_for_duplicate_names() {
        set_default_credential_builder(keyring::mock::default_credential_builder());

        let first = RxContainedRef::SavedSearch(Cow::Owned(RxSavedSearch::new(
            "same".into(),
            "query-b".into(),
            vec![],
        )));
        let second = RxContainedRef::SavedSearch(Cow::Owned(RxSavedSearch::new(
            "same".into(),
            "query-a".into(),
            vec![],
        )));

        let mut items = vec![first, second];
        sort_contained_refs(&mut items);
        let uuids: Vec<_> = items.iter().map(RxContainedRef::uuid).collect();

        assert!(uuids[0] < uuids[1]);
    }
}
