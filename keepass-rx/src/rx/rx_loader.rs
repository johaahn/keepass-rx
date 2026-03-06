use crate::crypto::MasterKey;

use super::{RxEntry, RxGroup, RxMetadata, RxSavedSearchDef, RxTemplate, ZeroableDatabase};
use anyhow::Result;
use indexmap::IndexMap;
use keepass::db::{Group, Icon, Meta as KeePassMeta, Node, Value};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::mem;
use std::rc::Rc;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

pub struct RxLoader {
    db: Zeroizing<ZeroableDatabase>,
    state: LoadState,
    master_key: Option<Rc<MasterKey>>,
    icons: HashMap<Uuid, Icon>,
}

pub struct Loaded {
    pub root_uuid: Uuid,
    pub state: LoadState,
    pub metadata: RxMetadata,
    pub saved_searches: Vec<RxSavedSearchDef>,
    pub master_key: Rc<MasterKey>,
}

fn parse_saved_searches(meta: &KeePassMeta) -> Vec<RxSavedSearchDef> {
    let raw_json = meta
        .custom_data
        .items
        .get("KPXC_SavedSearch")
        .and_then(|item| item.value.as_ref())
        .and_then(|value| match value {
            Value::Unprotected(value) => Some(value.to_string()),
            Value::Protected(value) => {
                Some(String::from_utf8_lossy(value.unsecure()).to_string())
            }
            Value::Bytes(_) => None,
        })
        .and_then(|decoded| serde_json::from_str::<JsonValue>(&decoded).ok())
        .and_then(|json| json.as_object().cloned())
        .unwrap_or_default();

    raw_json
        .into_iter()
        .filter_map(|(name, value)| {
            value.as_str().map(|query| RxSavedSearchDef {
                name,
                query: query.to_string(),
            })
        })
        .collect()
}

impl RxLoader {
    pub fn new(db: Zeroizing<ZeroableDatabase>) -> Self {
        Self {
            db: db,
            state: Default::default(),
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

        let config = mem::take(&mut self.db().config);
        let meta = mem::take(&mut self.db().meta);
        let saved_searches = parse_saved_searches(&meta);
        let rx_metadata = RxMetadata::new(config, meta);

        self.db.zeroize();

        Ok(Loaded {
            master_key: self.master_key.unwrap(),
            state: self.state,
            metadata: rx_metadata,
            saved_searches,
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
    use std::str::FromStr;

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
