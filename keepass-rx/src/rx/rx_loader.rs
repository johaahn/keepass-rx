use crate::crypto::MasterKey;

use super::icons::RxIcon;
use super::{RxDatabase, RxEntry, RxGroup, RxMetadata, RxTemplate, RxTotp, ZeroableDatabase};
use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use keepass::config::DatabaseConfig;
use keepass::db::{Group, Icon, Meta, Node};
use paste::paste;
use regex::Regex;
use std::mem;
use std::rc::Rc;
use std::sync::LazyLock;
use std::{collections::HashMap, str::FromStr};
use unicase::UniCase;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub struct RxLoader {
    db: Zeroizing<ZeroableDatabase>,
    state: LoadState,
    root: Option<RxGroup>,
    master_key: Option<Rc<MasterKey>>,
    icons: HashMap<Uuid, Icon>,
}

pub struct Loaded {
    pub root_uuid: Uuid,
    pub state: LoadState,
    pub metadata: RxMetadata,
    pub master_key: Rc<MasterKey>,
}

impl RxLoader {
    pub fn new(db: Zeroizing<ZeroableDatabase>) -> Self {
        Self {
            db: db,
            state: Default::default(),
            root: Default::default(),
            master_key: Default::default(),
            icons: Default::default(),
        }
    }

    fn db(&mut self) -> &mut Zeroizing<ZeroableDatabase> {
        &mut self.db
    }

    pub fn load(mut self) -> Result<Loaded> {
        self.icons = self
            .db()
            .meta
            .custom_icons
            .icons
            .iter()
            .map(|icon| (icon.uuid, icon.to_owned()))
            .collect();

        self.master_key = Some(Rc::new(
            MasterKey::new().expect("Could not create a master key"),
        ));

        self.state = LoadState::default();

        let mut db_root = mem::take(&mut self.db().root);

        let root_group = self.load_groups_recursive(&mut db_root, None);

        let root_uuid = root_group.uuid;
        self.state
            .all_groups
            .insert(root_group.uuid, Rc::new(root_group));

        let rx_metadata = RxMetadata::new(
            mem::take(&mut self.db().config),
            mem::take(&mut self.db().meta),
        );

        self.db.zeroize();

        Ok(Loaded {
            master_key: self.master_key.unwrap(),
            state: self.state,
            metadata: rx_metadata,
            root_uuid: root_uuid,
        })
    }

    /// Create RxGroup instances recursively, while simultaneously
    /// buildingd up the database state.
    pub fn load_groups_recursive(
        &mut self,
        group: &mut Group,
        parent_group_uuid: Option<Uuid>,
    ) -> RxGroup {
        let mut subgroups = vec![];
        let mut entries = vec![];

        let children = mem::take(&mut group.children);

        for node in children.into_iter() {
            match node {
                Node::Group(mut subgroup) => {
                    let subgroup_id = subgroup.uuid;
                    let rx_subgroup =
                        self.load_groups_recursive(&mut subgroup, Some(subgroup_id));

                    subgroups.push(rx_subgroup);
                }
                Node::Entry(entry) => {
                    let icon = entry
                        .custom_icon_uuid
                        .and_then(|icon_uuid| self.icons.get(&icon_uuid).cloned());

                    let rx_entry = RxEntry::new(
                        self.master_key.as_ref().unwrap(),
                        entry,
                        group.uuid,
                        icon,
                    );

                    // Build up template entries as we go. Name of the
                    // template will be set later, in RxDatabase::new.
                    if let Some(template_uuid) = rx_entry.template_uuid {
                        let rx_template = Rc::get_mut(
                            self.state.templates.entry(template_uuid).or_default(),
                        )
                        .expect("Could not update template");

                        rx_template.uuid = template_uuid;
                        rx_template.entry_uuids.push(rx_entry.uuid);
                    }

                    entries.push(rx_entry);
                }
            }
        }

        let this_group = RxGroup::new(
            group,
            subgroups.iter().map(|sg| sg.uuid).collect(),
            entries.iter().map(|e| e.uuid).collect(),
            parent_group_uuid,
        );

        for subgroup in subgroups {
            self.state
                .all_groups
                .insert(subgroup.uuid, Rc::new(subgroup));
        }

        for entry in entries {
            self.state.all_entries.insert(entry.uuid, Rc::new(entry));
        }

        this_group
    }
}

/// Various global-ish things to carry around during recursive
/// loading, that are returned to the final RxDatabase object.
#[derive(Default)]
pub struct LoadState {
    pub templates: HashMap<Uuid, Rc<RxTemplate>>,
    pub all_groups: IndexMap<Uuid, Rc<RxGroup>>,
    pub all_entries: IndexMap<Uuid, Rc<RxEntry>>,
}

#[cfg(test)]
mod tests {
    use keepass::Database;
    use keyring::set_default_credential_builder;

    use crate::rx::TEMPLATE_FIELD_NAME;

    use super::*;

    #[test]
    fn finds_templates() {
        set_default_credential_builder(keyring::mock::default_credential_builder());
        let template_uuid =
            Uuid::from_str("8e4ff17c-a985-4e50-abde-e641783464ca").expect("bad uuid");

        let mut group = keepass::db::Group::new("groupname");
        let mut entry = keepass::db::Entry::new();

        entry.fields.insert(
            TEMPLATE_FIELD_NAME.to_string(),
            keepass::db::Value::Unprotected(template_uuid.to_string()),
        );

        group.add_child(keepass::db::Node::Entry(entry));

        let mut loader = RxLoader::new(Zeroizing::new(ZeroableDatabase(Database::new(
            Default::default(),
        ))));

        loader.master_key = Some(Rc::new(
            MasterKey::new().expect("Could not make master key"),
        ));

        loader.load_groups_recursive(&mut group, None);

        assert_eq!(loader.state.templates.len(), 1);
        assert!(loader.state.templates.contains_key(&template_uuid));
    }
}
