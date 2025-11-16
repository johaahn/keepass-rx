use indexmap::{IndexMap, IndexSet};
use keepass::db::Group;
use std::{collections::HashMap, mem};
use uuid::Uuid;
use zeroize::Zeroize;

/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use super::{RxDatabase, RxEntry, RxGroup, RxTemplate, RxValue, icons::RxIcon};

#[derive(Clone)]
pub struct RxRoot {
    root_container: RxContainer,
    all_containers: IndexSet<Uuid>,
}

impl RxRoot {
    pub fn virtual_root(children: Vec<RxContainer>) -> Self {
        let all_child_uuids: IndexSet<Uuid> = children
            .iter()
            .flat_map(|child| child.all_children())
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

    pub fn with_db<'root, 'db>(&'root self, db: &'db RxDatabase) -> RxRootWithDb<'root, 'db> {
        RxRootWithDb::new(self, db)
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

    pub fn all_children(&self) -> IndexSet<Uuid> {
        let mut these_uuids = IndexSet::new();

        for child in self.children() {
            these_uuids.insert(child.uuid());

            for child_uuid in child.all_children() {
                these_uuids.insert(child_uuid);
            }
        }

        these_uuids
    }

    pub fn all_child_containers(&self) -> IndexMap<Uuid, &RxContainer> {
        let mut children = IndexMap::new();

        for child in self.children() {
            children.insert(child.uuid(), child);
            children.append(&mut child.all_child_containers());
        }

        children
    }

    pub fn find_child_recursive(&self, uuid: Uuid) -> Option<&RxContainer> {
        self.all_child_containers().get(&uuid).map(|v| &**v)
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

fn get_container(db: &RxDatabase, uuid: Uuid) -> Option<RxContainer> {
    db.get_group(uuid)
        .map(|group| RxContainer::from(group, db))
        .or_else(|| {
            db.get_template(uuid)
                .map(|template| RxContainer::from(template, db))
        })
        .or_else(|| db.get_entry(uuid).map(|ent| RxContainer::from(ent, db)))
}

impl IntoContainer for &RxGroup {
    fn into_container(&self, db: &RxDatabase) -> RxContainer {
        let children: Vec<_> = [self.subgroups.as_slice(), self.entries.as_slice()]
            .concat()
            .into_iter()
            .flat_map(|id| get_container(db, id))
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
            .flat_map(|id| get_container(db, *id))
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

pub struct RxRootWithDb<'root, 'db>
where
    'root: 'db,
{
    root: &'root RxRoot,
    all_containers: HashMap<Uuid, &'root RxContainer>,
    db: &'db RxDatabase,
}

impl<'root, 'db> RxRootWithDb<'root, 'db> {
    pub fn new(root: &'root RxRoot, db: &'db RxDatabase) -> Self {
        let all_containers: HashMap<_, _> = root
            .all_containers
            .iter()
            .flat_map(|uuid| {
                root.root_container
                    .find_child_recursive(*uuid)
                    .map(|container| (*uuid, container))
            })
            .collect();

        Self {
            root: root,
            all_containers: all_containers,
            db: db,
        }
    }

    pub fn db(&self) -> &'db RxDatabase {
        self.db
    }

    pub fn root(&self) -> RxContainerWithDb<'_> {
        RxContainerWithDb(&self.root.root_container, self.db())
    }

    pub fn get_container(&self, container_uuid: Uuid) -> Option<RxContainerWithDb<'db>> {
        self.all_containers
            .get(&container_uuid)
            .map(|container| RxContainerWithDb(&*container, self.db()))
    }

    pub fn children(&self) -> Vec<RxContainerWithDb<'_>> {
        let containers: Vec<_> = self.root.root_container.children().into_iter().collect();

        containers
            .into_iter()
            .map(|container| RxContainerWithDb(container, self.db()))
            .collect()
    }
}

/// A reference to the actual thing in the database, as pointed to by the container.
#[derive(Clone, Copy)]
pub enum RxContainedRef<'db> {
    Entry(&'db RxEntry),
    Group(&'db RxGroup),
    Template(&'db RxTemplate),
}

// We now need to move search logic into these virtual hierarchies.
// Right now the trait serves as basically a dumping ground for
// business logic. But this will not work. We tried carrying the
// source struct in. This was a bad idea. We instead need to carry the
// source as an enum or something. Or even better, somehow do an impl
// Search trait. We could tack a generic onto VirtualHierarchy that
// makes it return a struct associated with the RxRoot. If we make
// RxRoot contain a dyn Search impl, then we can use it?
//
// Or rather, we hold RxRoot AND search logic. Rework the
// VirtualHierarchy impls to not borrow DB. Instead, they hold an
// RxRoot, and we make functions that borrow DB to create the type.

/// An arbitrary hierarchical view into the password database.
pub trait VirtualHierarchy {
    fn root(&self) -> &RxRoot;
    fn search(
        &self,
        db: &RxDatabase, // TODO eventually move to own struct?,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'_>>;
}

#[derive(Clone)]
pub struct AllTemplates(RxRoot);

impl AllTemplates {
    pub fn new(db: &RxDatabase) -> Self {
        let children: Vec<_> = db
            .templates_iter()
            .map(|t| RxContainer::from(t, db))
            .collect();

        AllTemplates(RxRoot::virtual_root(children))
    }
}

impl VirtualHierarchy for AllTemplates {
    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn search(
        &self,
        db: &RxDatabase,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'_>> {
        // let woot = self.root().with_db(db);
        vec![]
    }
}

pub struct DefaultView(RxRoot);

impl DefaultView {
    pub fn new(db: &RxDatabase) -> Self {
        let root = RxContainer::from(db.root_group(), db);

        DefaultView(RxRoot {
            all_containers: root.all_children(),
            root_container: root,
        })
    }
}

impl VirtualHierarchy for DefaultView {
    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn search(
        &self,
        db: &RxDatabase,
        container_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Vec<RxContainedRef<'_>> {
        vec![]
    }
}
