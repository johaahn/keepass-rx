use gettextrs::{gettext, pgettext};
use indexmap::IndexSet;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};
use uuid::Uuid;

use super::{RxContainedRef, RxContainer, RxDatabase, RxRoot, RxTag};

/// A setting that controls how an RxListItem is rendered in the UI.
/// Note that the UI container of the list item must also have the
/// feature enabled for the feature to be enabled. This prevents,
/// e.g., rendering 2FA codes in the list outside of the 2FA codes
/// view.
#[cfg_attr(feature = "gui", derive(QEnum))]
#[derive(Clone, Default, Copy, PartialEq)]
#[repr(C)]
pub enum RxViewFeature {
    #[default]
    None,
    DisplayTwoFactorAuth,
}

impl std::fmt::Display for RxViewFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str_val = match self {
            RxViewFeature::DisplayTwoFactorAuth => "DisplayTwoFactorAuth",
            RxViewFeature::None => "None",
        };

        write!(f, "{}", str_val)
    }
}

/// An arbitrary hierarchical view into the password database. A
/// VirtualHierarchy manages two lifetimes when searching: the
/// lifetime of the RxContainer ('cnt) and the lifetime of the
/// RxDatabase ('db).
pub trait VirtualHierarchy {
    fn root(&self) -> &RxRoot;

    fn name(&self) -> String;

    fn feature(&self) -> RxViewFeature {
        RxViewFeature::None
    }

    fn get(&self, container_uuid: Uuid) -> Option<RxContainedRef> {
        self.root()
            .get_container(container_uuid)
            .and_then(|c| c.get_ref())
    }

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

        AllTemplates(RxRoot::virtual_root(
            &gettext("Special Categories"),
            children,
        ))
    }
}

impl VirtualHierarchy for AllTemplates {
    fn name(&self) -> String {
        gettext("All Templates")
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

    fn name(&self) -> String {
        pgettext(
            "Default view of the password database (all entries)",
            "Default View",
        )
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

        let root = RxRoot::virtual_root(
            &pgettext(
                "Two-factor/OTP codes (keep phrase short as possible)",
                "2FA Codes",
            ),
            entries,
        );

        TotpEntries(root)
    }
}

impl VirtualHierarchy for TotpEntries {
    fn name(&self) -> String {
        pgettext(
            "Two-factor/OTP codes (keep phrase short as possible)",
            "2FA Codes",
        )
    }

    fn root(&self) -> &RxRoot {
        &self.0
    }

    fn feature(&self) -> RxViewFeature {
        RxViewFeature::DisplayTwoFactorAuth
    }

    fn search(&self, container_uuid: Uuid, search_term: Option<&str>) -> Vec<RxContainedRef> {
        self.root()
            .get_container(container_uuid)
            .map(|container| container.search_children_immediate(search_term))
            .unwrap_or_default()
    }
}

pub struct AllTags(RxRoot);

impl AllTags {
    pub fn new(db: &RxDatabase) -> Self {
        let mut tags: HashMap<String, Vec<Uuid>> = HashMap::new();

        let tagged_entries = db.all_entries_iter().filter(|ent| ent.has_tags());

        for ent in tagged_entries {
            for tag in ent.as_ref().tags.as_slice() {
                let map_entry = tags.entry(tag.clone()).or_insert_with(|| vec![]);
                map_entry.push(ent.uuid);
            }
        }

        let children: Vec<_> = tags
            .into_iter()
            .map(|(tag, entry_uuids)| RxContainer::from(RxTag::new(tag, entry_uuids), db))
            .collect();

        let root =
            RxRoot::virtual_root(&pgettext("List of entries with tags", "Tags"), children);

        AllTags(root)
    }
}

impl VirtualHierarchy for AllTags {
    fn name(&self) -> String {
        gettext("Tags")
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
