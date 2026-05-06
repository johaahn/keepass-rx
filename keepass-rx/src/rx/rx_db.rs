use crate::crypto::MasterKey;

use super::rx_loader::RxLoader;
use super::{RxEntry, RxGroup, RxTemplate, RxTotp, ZeroableDatabase};
use anyhow::{Context, Result, anyhow};
use indexmap::IndexMap;
use keepass::config::DatabaseConfig;
use keepass::db::Meta;
use log::{debug, info};
use std::rc::Rc;
use std::{collections::HashMap, mem, str::FromStr};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

fn get_kpxc_string_field(config: &DatabaseConfig, field: &str) -> Option<String> {
    config
        .public_custom_data
        .as_ref()
        .and_then(|data| data.data.get(field))
        .and_then(|value| Into::<Option<&String>>::into(value).cloned())
}

fn get_kpxc_i32_field(config: &DatabaseConfig, field: &str) -> Option<i32> {
    config
        .public_custom_data
        .as_ref()
        .and_then(|data| data.data.get(field))
        .and_then(|value| Into::<Option<&i32>>::into(value).copied())
}

#[derive(Default, Clone)]
pub struct RxMetadata {
    pub color: Option<String>,
    pub name: Option<String>,
    pub icon: Option<i32>,
    pub recycle_bin_uuid: Option<Uuid>,
}

#[derive(Default, Clone)]
pub struct RxSavedSearchDef {
    pub name: String,
    pub query: String,
}

impl RxMetadata {
    pub fn new(config: DatabaseConfig, meta: Meta) -> RxMetadata {
        RxMetadata {
            name: get_kpxc_string_field(&config, "KPXC_PUBLIC_NAME"),
            color: get_kpxc_string_field(&config, "KPXC_PUBLIC_COLOR"),
            icon: get_kpxc_i32_field(&config, "KPXC_PUBLIC_ICON"),
            recycle_bin_uuid: meta.recyclebin_uuid,
        }
    }
}

#[derive(Clone)]
pub struct RxDatabase {
    // Not to be confused with encryption key for master DB password.
    // This is for encrypting values in memory.
    master_key: Rc<MasterKey>,
    metadata: RxMetadata,
    root: Uuid,
    templates: HashMap<Uuid, Rc<RxTemplate>>,
    all_groups: IndexMap<Uuid, Rc<RxGroup>>,
    all_entries: IndexMap<Uuid, Rc<RxEntry>>,
    saved_searches: Vec<RxSavedSearchDef>,
}

impl Zeroize for RxDatabase {
    fn zeroize(&mut self) {
        let _ = self.master_key.poison();

        let templates = self
            .templates
            .values_mut()
            .into_iter()
            .flat_map(|t| Rc::get_mut(t));

        let groups = self
            .all_groups
            .values_mut()
            .into_iter()
            .flat_map(|g| Rc::get_mut(g));

        let entries = self
            .all_entries
            .values_mut()
            .into_iter()
            .flat_map(|e| Rc::get_mut(e));

        for template in templates {
            template.zeroize();
        }

        for group in groups {
            group.zeroize();
        }

        for entry in entries {
            entry.zeroize();
        }
    }
}

// Manual impl because otherwise printing debug will dump the raw
// contents of the ENTIRE database and crash the terminal.
impl std::fmt::Debug for RxDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let groups_count = self.all_groups_iter().count();
        f.debug_struct("RxDatabase")
            .field("groups", &groups_count)
            .field(
                "entries",
                &self
                    .all_groups_iter()
                    .map(|group| group.entries.len())
                    .sum::<usize>(),
            )
            .finish()
    }
}

#[allow(dead_code)]
impl RxDatabase {
    pub fn new(db: Zeroizing<ZeroableDatabase>) -> Result<Self> {
        info!("Starting RxDatabase construction");
        let loader = RxLoader::new(db);
        let mut loaded = loader.load().context("materializing database view")?;

        let mut db = Self {
            master_key: loaded.master_key,
            root: loaded.root_uuid,
            templates: Default::default(),
            all_groups: loaded.state.all_groups,
            all_entries: loaded.state.all_entries,
            saved_searches: loaded.saved_searches,
            metadata: loaded.metadata,
        };

        // Map templates. Easier to do when we have access to DB logic.
        let rx_templates = loaded
            .state
            .templates
            .iter_mut()
            .map(|(uuid, t)| {
                Rc::get_mut(t)
                    .ok_or_else(|| anyhow!("Could not acquire mutable template ref for {uuid}"))
            })
            .collect::<Result<Vec<_>>>()?;

        for rx_template in rx_templates {
            let template_entry = db.get_entry(rx_template.uuid);
            let template_name = template_entry
                .as_ref()
                .and_then(|t| t.title().and_then(|v| v.value()))
                .map(|mut template_name| mem::take(&mut *template_name))
                .unwrap_or_else(|| "Unknown Template".to_string());

            let template_icon = template_entry
                .as_ref()
                .map(|t| t.icon.clone())
                .unwrap_or_default();

            rx_template.icon = template_icon;
            rx_template.name = template_name;
        }

        db.templates = loaded.state.templates;
        debug!(
            "Finished RxDatabase construction ({} groups, {} entries)",
            db.all_groups.len(),
            db.all_entries.len()
        );

        Ok(db)
    }

    pub fn master_key(&self) -> &MasterKey {
        &self.master_key
    }

    pub fn close(&mut self) {
        info!("Closing database.");
        self.zeroize();
    }

    pub fn metadata(&self) -> &RxMetadata {
        &self.metadata
    }

    pub fn root_group(&self) -> &Rc<RxGroup> {
        self.all_groups.get(&self.root).expect("No root group")
    }

    pub fn all_groups_iter(&self) -> impl Iterator<Item = &RxGroup> {
        self.all_groups.values().map(|g| g.as_ref())
    }

    pub fn all_entries_iter(&self) -> impl Iterator<Item = &Rc<RxEntry>> {
        self.all_entries.values()
    }

    pub fn get_group(&self, group_uuid: Uuid) -> Option<Rc<RxGroup>> {
        self.all_groups.get(&group_uuid).cloned()
    }

    pub fn get_entry(&self, entry_uuid: Uuid) -> Option<Rc<RxEntry>> {
        self.all_entries.get(&entry_uuid).cloned()
    }

    pub fn templates_iter(&self) -> impl Iterator<Item = &Rc<RxTemplate>> {
        self.templates.values()
    }

    pub fn get_template(&self, template_uuid: Uuid) -> Option<Rc<RxTemplate>> {
        self.templates.get(&template_uuid).cloned()
    }

    pub fn get_totp(&self, entry_uuid: &str) -> Result<RxTotp> {
        let entry_uuid = Uuid::from_str(entry_uuid)?;
        let entry = self
            .all_entries
            .get(&entry_uuid)
            .ok_or(anyhow!("Could not find OTP entry"))?;

        Ok(entry.totp()?)
    }

    pub fn saved_searches_iter(&self) -> impl Iterator<Item = &RxSavedSearchDef> {
        self.saved_searches.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use keepass::{Database, DatabaseKey};
    use keyring::set_default_credential_builder;

    use crate::rx::kpxc_search::evaluate_saved_search;

    use super::*;
    use crate::rx::{RxCustomFields, RxValue, TEMPLATE_FIELD_NAME};

    #[test]
    fn loads_recursively() {
        set_default_credential_builder(keyring::mock::default_credential_builder());
        let mut db = keepass::db::Database::new();
        let group_id = db.root().id().uuid();
        let (subgroup_id, entry_id) = {
            let mut root = db.root_mut();
            root.name = "groupname".into();

            let subgroup_id = root
                .add_group()
                .edit(|group| group.name = "subgroupname".into())
                .id();

            let entry_id = root
                .add_entry()
                .edit(|entry| {
                    entry.fields.insert(
                        "Tite".to_string(),
                        keepass::db::Value::Unprotected("top-level entry".to_string()),
                    );
                })
                .id();

            (subgroup_id, entry_id)
        };

        let sub_entry_id = db
            .group_mut(subgroup_id)
            .unwrap()
            .add_entry()
            .edit(|entry| {
                entry.fields.insert(
                    "SubTite".to_string(),
                    keepass::db::Value::Unprotected("sub entry".to_string()),
                );
            })
            .id();

        let subgroup_id = subgroup_id.uuid();
        let entry_id = entry_id.uuid();
        let sub_entry_id = sub_entry_id.uuid();

        let rx_db = RxDatabase::new(Zeroizing::new(ZeroableDatabase(db))).expect("load rx db");
        let rx_root = rx_db.root_group();

        assert_eq!(rx_db.all_groups_iter().count(), 2);
        assert_eq!(rx_db.all_entries_iter().count(), 2);

        assert_eq!(rx_root.entries.len(), 1);
        assert_eq!(rx_root.entries, vec![entry_id]);
        assert_eq!(rx_root.subgroups.len(), 1);
        assert_eq!(rx_root.uuid, group_id);
        assert_eq!(rx_root.subgroups, vec![subgroup_id]);

        let rx_subgroup = rx_db
            .get_group(subgroup_id)
            .expect("Could not find subgroup in DB");

        assert_eq!(rx_subgroup.uuid, subgroup_id);
        assert_eq!(rx_subgroup.entries, vec![sub_entry_id]);
    }

    #[test]
    fn parses_saved_searches_from_test_db() {
        set_default_credential_builder(keyring::mock::default_credential_builder());

        let mut file = File::open("test.kdbx").expect("open test.kdbx");
        let db = Database::open(&mut file, DatabaseKey::new().with_password("somePassw0rd"))
            .expect("open keepass db");

        let rx_db = RxDatabase::new(Zeroizing::new(ZeroableDatabase(db))).expect("load rx db");
        let saved: Vec<_> = rx_db.saved_searches_iter().collect();
        assert!(!saved.is_empty(), "expected at least one saved search");

        let account_search = saved
            .iter()
            .find(|search| search.name == "a saved search")
            .expect("missing expected saved search");

        assert_eq!(account_search.query, "account");

        let results = evaluate_saved_search(&rx_db, &account_search.query);
        assert!(
            !results.is_empty(),
            "saved search query should match at least one entry"
        );
    }

    // TODO move to rx_containers
    // #[test]
    // fn finds_entries_in_group() {
    //     set_default_credential_builder(keyring::mock::default_credential_builder());
    //     let master_key = Rc::new(MasterKey::new().expect("could not create master key"));
    //     let group_uuid =
    //         Uuid::from_str("133b7912-7705-4967-bc6e-807761ba9479").expect("bad group uuid");
    //     let entry_uuid =
    //         Uuid::from_str("d7e5dcb1-1e36-4b2a-9468-74f699809c1d").expect("bad entry uuid");

    //     let entry = RxEntry {
    //         uuid: entry_uuid,
    //         ..DefaultWithKey::default_with_key(&master_key)
    //     };

    //     let group = RxGroup {
    //         entries: vec![entry_uuid],
    //         name: "asdf".to_string(),
    //         subgroups: vec![],
    //         uuid: group_uuid,
    //         parent: None,
    //         icon: RxIcon::None,
    //     };

    //     let db = RxDatabase {
    //         root: group.uuid,
    //         templates: HashMap::new(),
    //         all_entries: IndexMap::from([(entry_uuid, entry)]),
    //         all_groups: IndexMap::from([(group_uuid, group)]),
    //         master_key: master_key,
    //         metadata: Default::default(),
    //     };

    //     let entries = db.get_entries(group_uuid, None);
    //     assert!(entries.len() > 0);
    //     assert_eq!(entries[0].uuid, entry_uuid);
    // }

    // TODO move to rx_containers
    // #[test]
    // fn search_entries_in_root_group() {
    //     set_default_credential_builder(keyring::mock::default_credential_builder());
    //     let master_key = Rc::new(MasterKey::new().expect("Could not create a master key"));

    //     let group_uuid =
    //         Uuid::from_str("133b7912-7705-4967-bc6e-807761ba9479").expect("bad group uuid");

    //     let entry_uuid1 =
    //         Uuid::from_str("d7e5dcb1-1e36-4b2a-9468-74f699809c1d").expect("bad entry uuid");

    //     let entry_uuid2 =
    //         Uuid::from_str("3c79f95c-842a-4e70-aed9-6cb7d128b01e").expect("bad entry uuid");

    //     let entry1 = RxEntry {
    //         uuid: entry_uuid1,
    //         title: Some(RxValue::try_from("test title".to_string()).expect("bad value")),
    //         ..DefaultWithKey::default_with_key(&master_key)
    //     };

    //     let entry2 = RxEntry {
    //         uuid: entry_uuid2,
    //         title: Some(
    //             RxValue::try_from("should not show up".to_string()).expect("bad value"),
    //         ),
    //         ..DefaultWithKey::default_with_key(&master_key)
    //     };

    //     let group = RxGroup {
    //         entries: vec![entry_uuid1, entry_uuid2],
    //         name: "asdf".to_string(),
    //         subgroups: vec![],
    //         uuid: group_uuid,
    //         parent: None,
    //         icon: RxIcon::None,
    //     };

    //     let db = RxDatabase {
    //         root: group.uuid,
    //         templates: HashMap::new(),
    //         all_entries: IndexMap::from([(entry_uuid1, entry1), (entry_uuid2, entry2)]),
    //         all_groups: IndexMap::from([(group_uuid, group)]),
    //         master_key: master_key,
    //         metadata: Default::default(),
    //     };

    //     let entries = db.get_entries(group_uuid, Some("test"));

    //     assert!(
    //         entries.len() == 1,
    //         "expected 1 entry, but got {}",
    //         entries.len()
    //     );
    //     assert_eq!(entries[0].uuid, entry_uuid1);
    // }
}
