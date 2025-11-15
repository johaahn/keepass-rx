use keepass::db::Group;
use std::mem;
use uuid::Uuid;
use zeroize::Zeroize;

/// An RxContainer is a virtual hierarchy of group-like containers and
/// child entries. A container can have any number of child containers
/// (of the same type) and any number of RxEntry objects.
use super::{RxDatabase, RxEntry, RxGroup, RxTemplate, RxValue, icons::RxIcon};

pub struct RxContainerRoot<'a> {
    root: RxContainer<'a>,
    children: Vec<RxContainer<'a>>,
}

impl<'a> RxContainerRoot<'a> {
    pub fn children(&self) -> &[RxContainer<'a>] {
        self.children.as_slice()
    }
}

#[derive(Clone, Copy)]
pub struct RxContainer<'a> {
    item: RxContainerItem<'a>,
    is_root: bool,
}

impl<'a> RxContainer<'a> {
    pub fn from<T>(item: T, is_root: bool) -> Self
    where
        T: Into<RxContainerItem<'a>>,
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

    pub fn item(&self) -> &RxContainerItem<'a> {
        &self.item
    }
}

#[derive(Clone, Copy)]
pub enum RxContainerItem<'a> {
    Grouping(RxContainerGrouping<'a>),
    Entry(&'a RxEntry),
}

impl<'a> RxContainerItem<'a> {
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Grouping(grouping) => grouping.uuid(),
            Self::Entry(entry) => entry.uuid,
        }
    }
}

impl<'a> From<&'a RxEntry> for RxContainerItem<'a> {
    fn from(value: &'a RxEntry) -> Self {
        Self::Entry(value)
    }
}

impl<'a> From<&'a RxGroup> for RxContainerItem<'a> {
    fn from(value: &'a RxGroup) -> Self {
        Self::Grouping(RxContainerGrouping::Group(value))
    }
}

impl<'a> From<&'a RxTemplate> for RxContainerItem<'a> {
    fn from(value: &'a RxTemplate) -> Self {
        Self::Grouping(RxContainerGrouping::Template(value))
    }
}

#[derive(Clone, Copy)]
pub enum RxContainerGrouping<'a> {
    Group(&'a RxGroup),
    Template(&'a RxTemplate),
}

#[allow(dead_code)]
impl RxContainerGrouping<'_> {
    pub fn uuid(&self) -> Uuid {
        match self {
            RxContainerGrouping::Group(group) => group.uuid,
            RxContainerGrouping::Template(template) => template.uuid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            RxContainerGrouping::Group(group) => &group.name,
            RxContainerGrouping::Template(template) => &template.name,
        }
    }

    pub fn children(&self) -> Vec<Uuid> {
        match self {
            RxContainerGrouping::Group(group) => {
                [group.subgroups.as_slice(), group.entries.as_slice()].concat()
            }
            RxContainerGrouping::Template(tmplt) => tmplt.entry_uuids.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RxContainerWithDb<'db>(RxContainer<'db>, &'db RxDatabase);

impl<'db> RxContainerWithDb<'db> {
    pub fn new(container: RxContainer<'db>, db: &'db RxDatabase) -> Self {
        Self(container, db)
    }

    pub fn container(&self) -> &RxContainer<'db> {
        &self.0
    }

    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn uuid(&self) -> Uuid {
        self.container().uuid()
    }

    pub fn find_children(&self, search_term: Option<&str>) -> Vec<RxContainer<'db>> {
        let container = self.container();
        let item = container.item();

        match item {
            &RxContainerItem::Grouping(RxContainerGrouping::Template(_))
                if container.is_root() =>
            {
                self.db()
                    .find_templates(search_term)
                    .map(|tmplt| RxContainer::from(tmplt, false))
                    .collect()
            }
            &RxContainerItem::Grouping(RxContainerGrouping::Template(tmplt))
                if !container.is_root() =>
            {
                self.db()
                    .get_template(tmplt.uuid)
                    .map(|template| {
                        self.1
                            .entries_iter_by_uuid(template.entry_uuids.as_slice(), search_term)
                            .map(|ent| RxContainer::from(ent, false))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }
            &RxContainerItem::Grouping(RxContainerGrouping::Group(group)) => {
                let subgroups_iter = self.1.filter_subgroups(group.uuid, search_term);
                let entries = self.1.get_entries(group.uuid, search_term);

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
        }
    }
}

pub struct RxRootWithDb<'db>(RxContainerRoot<'db>, &'db RxDatabase);

impl<'db> RxRootWithDb<'db> {
    pub fn db(&self) -> &'db RxDatabase {
        self.1
    }

    pub fn root(&self) -> RxContainerWithDb<'db> {
        RxContainerWithDb(self.0.root, self.db())
    }

    pub fn children(&self) -> Vec<RxContainerWithDb<'db>> {
        self.0
            .children()
            .into_iter()
            .map(|container| RxContainerWithDb(*container, self.db()))
            .collect()
    }
}
