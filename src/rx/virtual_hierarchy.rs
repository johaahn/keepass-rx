use std::rc::Rc;

use uuid::Uuid;

use super::{RxContainedRef, RxContainer, RxDatabase, RxRoot};

/// An arbitrary hierarchical view into the password database. A
/// VirtualHierarchy manages two lifetimes when searching: the
/// lifetime of the RxContainer ('cnt) and the lifetime of the
/// RxDatabase ('db).
pub trait VirtualHierarchy {
    fn root(&self) -> &RxRoot;

    fn name(&self) -> &str;

    /// Search for child containers in the virtual hierarchy.
    fn search(&self, container_uuid: Uuid, search_term: Option<&str>) -> Vec<RxContainedRef>;
}

#[derive(Clone)]
pub struct AllTemplates(RxRoot);

impl AllTemplates {
    pub fn new(db: &RxDatabase) -> Self {
        let children: Vec<_> = db
            .templates_iter()
            .map(|t| RxContainer::from(t.clone(), db))
            .collect();

        AllTemplates(RxRoot::virtual_root("Special Categories", children))
    }
}

impl VirtualHierarchy for AllTemplates {
    fn name(&self) -> &str {
        "All Templates"
    }

    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn search(&self, container_uuid: Uuid, search_term: Option<&str>) -> Vec<RxContainedRef> {
        if container_uuid == self.root().uuid() {
            // searching from the (non existant) "root template"
            // means we should search all templates instead.
            self.root()
                .root_container
                .search_children_immediate(search_term)
        } else {
            // Otherwise, we search inside the template itself
            let maybe_template = self.root().get_container(container_uuid);

            maybe_template
                .as_ref()
                .map(|tmplt| tmplt.search_children_immediate(search_term))
                .unwrap_or_default()
        }
    }
}

pub struct DefaultView(RxRoot);

impl DefaultView {
    pub fn new(db: &RxDatabase) -> Self {
        let root = RxContainer::from(db.root_group().clone(), db);

        DefaultView(RxRoot {
            all_containers: root.child_uuids_recursive(),
            root_container: Rc::new(root),
        })
    }
}

impl VirtualHierarchy for DefaultView {
    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn name(&self) -> &str {
        "Default View"
    }

    fn search(&self, container_uuid: Uuid, search_term: Option<&str>) -> Vec<RxContainedRef> {
        self.root()
            .get_container(container_uuid)
            .map(|container| container.search_children_immediate(search_term))
            .unwrap_or_default()
    }
}

pub struct TotpEntries(RxRoot);

impl TotpEntries {
    pub fn new(db: &RxDatabase) -> Self {
        let entries: Vec<_> = db
            .all_entries_iter()
            .filter(|ent| ent.has_otp())
            .cloned()
            .map(|ent| RxContainer::from(ent, db))
            .collect();

        let root = RxRoot::virtual_root("2FA Codes", entries);

        TotpEntries(root)
    }
}

impl VirtualHierarchy for TotpEntries {
    fn name(&self) -> &str {
        "2FA Entries"
    }

    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn search(&self, container_uuid: Uuid, search_term: Option<&str>) -> Vec<RxContainedRef> {
        self.root()
            .get_container(container_uuid)
            .map(|container| container.search_children_immediate(search_term))
            .unwrap_or_default()
    }
}
