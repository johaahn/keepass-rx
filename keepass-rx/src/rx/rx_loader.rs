use crate::crypto::MasterKey;

use super::{RxEntry, RxGroup, RxMetadata, RxSavedSearchDef, RxTemplate, ZeroableDatabase};
use anyhow::Result;
use indexmap::IndexMap;
use keepass::db::{
    CustomDataValue, GroupId, Meta as KeePassMeta,
};
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
        .get("KPXC_SavedSearch")
        .and_then(|item| item.value.as_ref())
        .and_then(|value| match value {
            CustomDataValue::String(value) => Some(value.to_string()),
            _ => None,
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
        }
    }

    fn db(&mut self) -> &mut Zeroizing<ZeroableDatabase> {
        &mut self.db
    }

    pub fn load(mut self) -> Result<Loaded> {
        self.master_key = Some(Rc::new(
            MasterKey::new().expect("Could not create a master key"),
        ));

        self.state = LoadState::default();

        let root_group = self.load_groups_recursive(self.db.root().id(), None);

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
        group_id: GroupId,
        parent_group_uuid: Option<Uuid>,
    ) -> RxGroup {
        let Some(group) = self.db.group(group_id) else {
            return RxGroup::default();
        };

        let child_group_ids: Vec<_> = group.group_ids().into_iter().collect();
        let child_entry_ids: Vec<_> = group.entry_ids().into_iter().collect();
        drop(group);

        let mut subgroups = Vec::new();
        for subgroup_id in child_group_ids {
            let rx_subgroup = self.load_groups_recursive(subgroup_id, Some(group_id.uuid()));
            subgroups.push(rx_subgroup);
        }

        let mut entries = Vec::new();
        for entry_id in child_entry_ids {
            let Some(entry) = self.db.entry_mut(entry_id) else {
                continue;
            };

            let rx_entry = RxEntry::new(self.master_key.as_ref().unwrap(), entry, group_id.uuid());

            // Build up template entries as we go. Name of the
            // template will be set later, in RxDatabase::new.
            if let Some(template_uuid) = rx_entry.template_uuid {
                let rx_template =
                    Rc::get_mut(self.state.templates.entry(template_uuid).or_default())
                        .expect("Could not update template");

                rx_template.uuid = template_uuid;
                rx_template.entry_uuids.push(rx_entry.uuid);
            }

            entries.push(rx_entry);
        }

        let Some(group) = self.db.group_mut(group_id) else {
            return RxGroup::default();
        };

        // Only need group id, name, icon. Extract to struct.
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
