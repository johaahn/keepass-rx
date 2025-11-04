use super::ZeroableDatabase;
use anyhow::{Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use humanize_duration::Truncate;
use humanize_duration::prelude::DurationExt;
use infer;
use keepass::db::{CustomData, Entry, Group, Icon, Node, TOTP as KeePassTOTP, Value};
use libsodium_rs::utils::{SecureVec, vec_utils};
use querystring::querify;
use secstr::SecStr;
use std::collections::HashSet;
use std::mem;
use std::{collections::HashMap, str::FromStr};
use totp_rs::{Secret, TOTP};
use unicase::UniCase;
use uriparse::URI;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

// Special field that indicates the entry is a templated entry (e.g.
// credit card, wifi password, etc).
const TEMPLATE_FIELD_NAME: &str = "_etm_template_uuid";

// Fields inserted by other KeePass programs that we do not want to
// show as custom fields. They might be used for other things in the
// app, though.
const FIELDS_TO_HIDE: [&str; 3] = [
    // KeePassXC browser integration (list of URLs)
    "KeePassXC-Browser Settings",
    // Last modified date
    "_LAST_MODIFIED",
    // UUID of a template (e.g. credit card entry), created by
    // KeePassDX.
    TEMPLATE_FIELD_NAME,
];

// Like FIELDS_TO_HIDE, but does a starts_with check to see if the
// value should be hidden.
const WILDCARD_FIELDS_TO_HIDE: [&str; 2] = ["AndroidApp", "KP2A_URL"];

macro_rules! expose {
    ($secret:expr) => {{
        $secret
            .as_ref()
            .and_then(|secret| secret.value().map(|value| value.to_string()))
            .unwrap_or_default()
    }};
}

macro_rules! expose_opt {
    ($secret:expr) => {{ $secret.as_ref().and_then(|secret| secret.value()) }};
}

macro_rules! expose_str {
    ($secret:expr) => {{
        $secret
            .as_ref()
            .and_then(|secret| secret.value())
            .unwrap_or_default()
    }};
}

macro_rules! into_value {
    ($key:expr, $val:expr) => {{ RxValue::try_from($val).ok().map(|secret| ($key, secret)) }};
}

pub(crate) use {expose, expose_opt, expose_str};

#[derive(Zeroize, Default, Clone)]
pub struct RxTotp {
    pub code: String,
    pub valid_for: String,
}

#[derive(Zeroize, Default, Clone)]
pub struct RxGroup {
    #[zeroize(skip)]
    pub uuid: Uuid,

    /// The parent UUID will be None if this is the root group.
    #[zeroize(skip)]
    pub parent: Option<Uuid>,

    pub name: String,

    // TODO use some kind of map instead of Vec (since maps do not
    // impl Zeroize). should be possible once we stop loading secrets
    // into memory.
    pub subgroups: Vec<RxGroup>,

    // TODO use some kind of map instead of Vec (since maps do not
    // impl Zeroize). should be possible once we stop loading secrets
    // into memory.
    pub entries: Vec<RxEntry>,
}

impl RxGroup {
    pub fn new(group: &mut Group, subgroups: Vec<RxGroup>, entries: Vec<RxEntry>) -> Self {
        Self {
            uuid: group.uuid,
            name: mem::take(&mut group.name),
            subgroups: subgroups,
            entries: entries,
            parent: None,
        }
    }
}

fn should_hide_field(field_name: &str) -> bool {
    FIELDS_TO_HIDE.contains(&field_name)
        || WILDCARD_FIELDS_TO_HIDE
            .iter()
            .any(|wildcard| field_name.starts_with(wildcard))
}

#[derive(Zeroize, Default, Clone)]
pub struct RxCustomFields(pub(crate) Vec<(String, RxValue)>);

impl RxCustomFields {
    pub fn append(&mut self, other: &mut RxCustomFields) {
        self.0.append(&mut other.0);
    }
}

impl From<Vec<(String, RxValue)>> for RxCustomFields {
    fn from(value: Vec<(String, RxValue)>) -> Self {
        let custom_fields: Vec<_> = value
            .into_iter()
            .flat_map(|(key, item)| {
                if !should_hide_field(&key.as_ref()) {
                    Some((key, item))
                } else {
                    None
                }
            })
            .collect();

        Self(custom_fields)
    }
}

impl From<CustomData> for RxCustomFields {
    fn from(value: CustomData) -> Self {
        let custom_fields: Vec<_> = value
            .items
            .into_iter()
            .flat_map(|(key, item)| {
                if !should_hide_field(&key.as_ref()) {
                    item.value.and_then(|value| match value {
                        Value::Protected(val) => into_value!(key, val),
                        Value::Unprotected(val) => into_value!(key, val),
                        _ => None,
                    })
                } else {
                    None
                }
            })
            .collect();

        Self(custom_fields)
    }
}

pub enum RxFieldName {
    Title,
    Username,
    Password,
    Url,
    CustomField(String),
}

impl ToString for RxFieldName {
    fn to_string(&self) -> String {
        match self {
            RxFieldName::Username => "Username".to_string(),
            RxFieldName::Password => "Password".to_string(),
            RxFieldName::Title => "Title".to_string(),
            RxFieldName::Url => "URL".to_string(),
            RxFieldName::CustomField(name) => name.to_owned(),
        }
    }
}

impl From<String> for RxFieldName {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_ref() {
            "username" => RxFieldName::Username,
            "password" => RxFieldName::Password,
            "title" => RxFieldName::Title,
            "url" => RxFieldName::Url,
            _ => RxFieldName::CustomField(value),
        }
    }
}

/// Not Sync or Send, because of SecureVec using *mut u8.
#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub enum RxValue {
    /// Fully hidden by default. Used for passwords etc.
    Protected(SecureVec<u8>),

    /// Not hidden by default, but we don't want to leak the metadata
    /// in memory if possible.
    Sensitive(SecureVec<u8>),

    /// Regular value not hidden or treated specially by memory
    /// protection.
    Unprotected(String),

    #[default]
    Unsupported,
}

impl RxValue {
    pub fn is_hidden_by_default(&self) -> bool {
        match self {
            RxValue::Protected(_) => true,
            _ => false,
        }
    }

    pub fn value(&self) -> Option<&str> {
        use RxValue::*;
        match self {
            Protected(val) | Sensitive(val) => std::str::from_utf8(&val).ok(),
            Unprotected(value) => Some(value.as_ref()),
            _ => None,
        }
    }
}

impl std::fmt::Display for RxValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RxValue::Protected(_) | RxValue::Sensitive(_) => write!(f, "**SECRET**"),
            RxValue::Unprotected(value) => write!(f, "{}", value),
            _ => write!(f, "<unsupported value>"),
        }
    }
}

impl std::fmt::Debug for RxValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RxValue::Protected(_) | RxValue::Sensitive(_) => write!(f, "**SECRET**"),
            RxValue::Unprotected(value) => write!(f, "{}", value),
            _ => write!(f, "<unsupported value>"),
        }
    }
}

impl TryFrom<SecStr> for RxValue {
    type Error = anyhow::Error;
    fn try_from(mut value: SecStr) -> std::result::Result<Self, Self::Error> {
        let value_unsecure = value.unsecure();
        let mut secure_vec = vec_utils::secure_vec::<u8>(value_unsecure.len())?;
        secure_vec.copy_from_slice(&value_unsecure);
        value.zero_out();
        Ok(RxValue::Protected(secure_vec))
    }
}

impl TryFrom<String> for RxValue {
    type Error = anyhow::Error;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let mut secure_vec = vec_utils::secure_vec::<u8>(value.len())?;
        secure_vec.copy_from_slice(value.as_ref());
        Ok(RxValue::Sensitive(secure_vec))
    }
}

#[derive(Zeroize, Default, Clone)]
pub struct RxEntry {
    #[zeroize(skip)]
    pub uuid: Uuid,

    /// An entry always has a parent group.
    #[zeroize(skip)]
    pub parent_group: Uuid,

    #[zeroize(skip)]
    pub template_uuid: Option<Uuid>,

    pub title: Option<RxValue>,
    pub username: Option<RxValue>,
    pub password: Option<RxValue>,
    pub notes: Option<RxValue>,

    // A map would be better, but it's not zeroizable.
    pub custom_fields: RxCustomFields,

    pub url: Option<RxValue>,
    pub raw_otp_value: Option<RxValue>,
    pub icon_data: Option<Vec<u8>>,
}

fn extract_remaining_fields(entry: &mut Entry) -> Vec<(String, RxValue)> {
    entry
        .fields
        .drain()
        .flat_map(|(key, value)| {
            match value {
                Value::Protected(val) => {
                    RxValue::try_from(val).ok().map(|rx_val| (key, rx_val))
                }
                Value::Unprotected(val) => {
                    RxValue::try_from(val).ok().map(|rx_val| (key, rx_val))
                }
                _ => None, // discard binary for now. attachments later?
            }
        })
        .collect()
}

fn extract_value(entry: &mut Entry, field_name: &str) -> Option<RxValue> {
    entry.fields.remove(field_name).and_then(|value| {
        match value {
            Value::Protected(val) => RxValue::try_from(val).ok(),
            Value::Unprotected(val) => RxValue::try_from(val).ok(),
            _ => None, // discard binary for now. attachments later?
        }
    })
}

#[allow(dead_code)]
impl RxEntry {
    pub fn new_without_icon(entry: Entry, parent_uuid: Uuid) -> Self {
        Self::new(entry, parent_uuid, None)
    }

    pub fn new(mut entry: Entry, parent_uuid: Uuid, icon: Option<Icon>) -> Self {
        let custom_data = mem::take(&mut entry.custom_data);

        let title = extract_value(&mut entry, "Title");
        let username = extract_value(&mut entry, "UserName");
        let password = extract_value(&mut entry, "Password");
        let notes = extract_value(&mut entry, "Notes");
        let url = extract_value(&mut entry, "URL");
        let raw_otp_value = extract_value(&mut entry, "otp");

        // Template: Extract the _etm_template_uuid field, which
        // points to a template DB entry.
        let template_uuid = extract_value(&mut entry, TEMPLATE_FIELD_NAME).and_then(|val| {
            val.value()
                .and_then(|uuid_str| Uuid::from_str(uuid_str).ok())
        });

        // Has to come after the above, otherwise those fields end up
        // here.
        let mut other_fields = RxCustomFields::from(extract_remaining_fields(&mut entry));
        let mut custom_fields = RxCustomFields::from(custom_data);
        custom_fields.append(&mut other_fields);

        Self {
            uuid: entry.uuid,
            parent_group: parent_uuid,
            template_uuid: template_uuid,
            title: title,
            username: username,
            password: password,
            notes: notes,
            custom_fields: custom_fields,
            url: url,
            raw_otp_value: raw_otp_value,
            icon_data: icon.map(|i| i.data),
        }
    }

    pub fn get_field_value(&self, field_name: &RxFieldName) -> Option<&RxValue> {
        match field_name {
            RxFieldName::Username => self.username.as_ref(),
            RxFieldName::Password => self.password.as_ref(),
            RxFieldName::Url => self.url.as_ref(),
            RxFieldName::Title => self.title.as_ref(),
            RxFieldName::CustomField(name) => {
                self.custom_fields
                    .0
                    .iter()
                    .find_map(|(key, value)| match key == name {
                        true => Some(value),
                        false => None,
                    })
            }
        }
    }

    pub fn has_steam_otp(&self) -> bool {
        expose!(self.raw_otp_value).starts_with("otpauth://totp/Steam:")
    }

    pub fn steam_otp_digits(&self) -> Result<String> {
        if !self.has_steam_otp() {
            return Err(anyhow!("Not a Steam OTP entry"));
        }

        let uri = URI::try_from(expose_str!(self.raw_otp_value))?;

        let query = uri
            .query()
            .ok_or(anyhow!("No querystring for Steam OTP entry"))?;

        let query_values = querify(query);

        let secret = query_values
            .into_iter()
            .find_map(|(key, value)| match key {
                "secret" => Some(value.to_string()),
                _ => None,
            })
            .ok_or(anyhow!("No Steam secret in OTP"))?;

        let steam_otp_code =
            TOTP::new_steam(Secret::Encoded(secret).to_bytes()?).generate_current()?;

        Ok(steam_otp_code)
    }

    pub fn totp(&self) -> Result<RxTotp> {
        let otp = KeePassTOTP::from_str(expose_str!(self.raw_otp_value))?;

        let otp_code = otp.value_now()?;

        let otp_digits = match self.has_steam_otp() {
            true => self.steam_otp_digits()?,
            false => otp_code.code,
        };

        let otp_valid_for = format!("{}", otp_code.valid_for.human(Truncate::Second));

        Ok(RxTotp {
            code: otp_digits,
            valid_for: otp_valid_for,
        })
    }

    pub fn icon_data_url(&self) -> Option<String> {
        self.icon_data.as_deref().and_then(|data| {
            infer::get(data).map(|k| {
                format!(
                    "data:{};base64,{}",
                    k.mime_type(),
                    BASE64_STANDARD.encode(data)
                )
            })
        })
    }
}

#[derive(Zeroize, Default, Clone, Hash, Eq, PartialEq)]
pub struct RxTemplate {
    #[zeroize(skip)]
    uuid: Uuid,
    name: String, // from the template's entry title.

    #[zeroize(skip)]
    entry_uuids: Vec<Uuid>,
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxDatabase {
    root: RxGroup,
    #[zeroize(skip)]
    templates: HashMap<Uuid, RxTemplate>,
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

/// Various global-ish things to carry around during recursive
/// loading, that are returned to the final RxDatabase object.
#[derive(Default)]
struct LoadState {
    templates: HashMap<Uuid, RxTemplate>,
}

fn load_groups_recursive(
    group: &mut Group,
    icons: &mut HashMap<Uuid, Icon>,
    state: &mut LoadState,
) -> Vec<RxGroup> {
    let this_group_id = group.uuid;
    let mut groups = vec![];
    let mut subgroups = vec![];
    let mut entries = vec![];

    let children = mem::take(&mut group.children);

    for node in children.into_iter() {
        match node {
            Node::Group(mut subgroup) => {
                let mut rx_groups: Vec<_> = load_groups_recursive(&mut subgroup, icons, state)
                    .into_iter()
                    .map(|mut subgroup| {
                        subgroup.parent = Some(this_group_id);
                        subgroup
                    })
                    .collect();

                subgroups.append(&mut rx_groups);
            }
            Node::Entry(entry) => {
                let icon = entry
                    .custom_icon_uuid
                    .and_then(|icon_uuid| icons.remove(&icon_uuid));

                let rx_entry = RxEntry::new(entry, group.uuid, icon);

                // Build up template entries as we go. Name of the
                // template will be set later, in RxDatabase::new.
                if let Some(template_uuid) = rx_entry.template_uuid {
                    let rx_template = state.templates.entry(template_uuid).or_default();

                    rx_template.uuid = template_uuid;
                    rx_template.entry_uuids.push(rx_entry.uuid);
                }

                entries.push(rx_entry);
            }
        }
    }

    groups.push(RxGroup::new(group, subgroups, entries));
    groups
}

struct RxGroupIter<'a> {
    stack: Vec<&'a RxGroup>,
}

impl<'a> Iterator for RxGroupIter<'a> {
    type Item = &'a RxGroup;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // Push children in reverse so they come out in original order.
        for child in node.subgroups.iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

impl RxDatabase {
    pub fn new(mut db: Zeroizing<ZeroableDatabase>) -> Self {
        let mut icons: HashMap<Uuid, Icon> = db
            .meta
            .custom_icons
            .icons
            .iter()
            .map(|icon| (icon.uuid, icon.to_owned()))
            .collect();

        // There should only be one group in the vec, which is the
        // root.
        let mut state = LoadState::default();
        let mut rx_groups = load_groups_recursive(&mut db.root, &mut icons, &mut state);
        let root_group = rx_groups.swap_remove(0);

        drop(db);

        let mut db = Self {
            root: root_group,
            templates: Default::default(),
        };

        // Map templates. Easier to do when we have access to DB logic.
        for (_, rx_template) in state.templates.iter_mut() {
            let template_name = db
                .get_entry(rx_template.uuid)
                .and_then(|t| t.title.as_ref().and_then(|v| v.value()))
                .map(|template_name| template_name.to_string())
                .unwrap_or_else(|| "Unknown Template".to_string());

            rx_template.name = template_name;
        }

        db.templates = state.templates;

        db
    }

    pub fn close(&mut self) {
        println!("Closing database.");
        self.zeroize();
    }

    pub fn root_group(&self) -> &RxGroup {
        &self.root
    }

    pub fn all_groups_iter(&self) -> impl Iterator<Item = &RxGroup> {
        RxGroupIter {
            stack: vec![&self.root],
        }
    }

    pub fn all_entries_iter(&self) -> impl Iterator<Item = &RxEntry> {
        self.all_groups_iter()
            .flat_map(|group| group.entries.iter())
    }

    pub fn get_group(&self, group_uuid: Uuid) -> Option<&RxGroup> {
        self.all_groups_iter()
            .find(|group| group.uuid == group_uuid)
    }

    pub fn get_group_filter_subgroups(
        &self,
        group_uuid: Uuid,
        search_term: Option<&str>,
    ) -> Option<RxGroup> {
        let mut maybe_group = self
            .all_groups_iter()
            .find(|group| group.uuid == group_uuid)
            .cloned();

        if let Some(group) = &mut maybe_group {
            group.subgroups.retain(|subgroup| match search_term {
                Some(term) => subgroup.name.to_lowercase().contains(term),
                None => true,
            });
        }

        maybe_group
    }

    pub fn get_entry(&self, entry_uuid: Uuid) -> Option<&RxEntry> {
        self.all_entries_iter()
            .find(|entry| entry.uuid == entry_uuid)
    }

    pub fn get_templates(&self) -> Vec<&RxEntry> {
        //
        vec![]
    }

    pub fn get_entries(&self, group_uuid: Uuid, search_term: Option<&str>) -> Vec<&RxEntry> {
        let search_term = search_term.map(|term| UniCase::new(term).to_folded_case());
        let group = self.get_group(group_uuid);
        let entries_in_group = group.as_ref().map(|group| group.entries.as_slice());

        // Determine if an entry should show up in search results, if
        // a search term was specified. term is already lowecase here.
        let search_entry = |entry: &RxEntry, term: &str| {
            let username = entry.username.as_ref().and_then(|u| {
                u.value()
                    .map(|secret| UniCase::new(secret).to_folded_case())
            });

            let url = entry.url.as_ref().and_then(|u| {
                u.value()
                    .map(|secret| UniCase::new(secret).to_folded_case())
            });

            let title = entry.title.as_ref().and_then(|u| {
                u.value()
                    .map(|secret| UniCase::new(secret).to_folded_case())
            });

            let contains_username = username.map(|u| u.contains(term)).unwrap_or(false);
            let contains_url = url.map(|u| u.contains(term)).unwrap_or(false);
            let contains_title = title.map(|t| t.contains(term)).unwrap_or(false);

            contains_username || contains_url || contains_title
        };

        let filtered_by_search = entries_in_group.map(|entries| {
            entries
                .into_iter()
                .filter_map(|entry| {
                    if let Some(term) = &search_term {
                        match search_entry(&entry, term) {
                            true => Some(entry),
                            false => None,
                        }
                    } else {
                        Some(entry)
                    }
                })
                .collect::<Vec<_>>()
        });

        filtered_by_search.unwrap_or_default()
    }

    pub fn get_totp(&self, entry_uuid: &str) -> Result<RxTotp> {
        let entry_uuid = Uuid::from_str(entry_uuid)?;
        let entry = self
            .all_groups_iter()
            .flat_map(|group| group.entries.as_slice())
            .find(|&entry| entry.uuid == entry_uuid);

        let otp = entry.map(|ent| ent.totp()).transpose();

        match otp {
            Ok(Some(otp)) => Ok(otp),
            Ok(None) => Err(anyhow!("Could not find OTP entry")),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_templates() {
        let template_uuid =
            Uuid::from_str("8e4ff17c-a985-4e50-abde-e641783464ca").expect("bad uuid");

        let mut group = keepass::db::Group::new("groupname");
        let mut entry = keepass::db::Entry::new();

        entry.fields.insert(
            TEMPLATE_FIELD_NAME.to_string(),
            keepass::db::Value::Unprotected(template_uuid.to_string()),
        );

        group.add_child(keepass::db::Node::Entry(entry));

        let mut state = LoadState::default();
        load_groups_recursive(&mut group, &mut HashMap::new(), &mut state);

        assert_eq!(state.templates.len(), 1);
        assert!(state.templates.contains_key(&template_uuid));
    }

    #[test]
    fn finds_entries_in_group() {
        let group_uuid =
            Uuid::from_str("133b7912-7705-4967-bc6e-807761ba9479").expect("bad group uuid");
        let entry_uuid =
            Uuid::from_str("d7e5dcb1-1e36-4b2a-9468-74f699809c1d").expect("bad entry uuid");

        let entry = RxEntry {
            uuid: entry_uuid,
            ..Default::default()
        };

        let group = RxGroup {
            entries: vec![entry],
            name: "asdf".to_string(),
            subgroups: vec![],
            uuid: group_uuid,
            parent: None,
        };

        let db = RxDatabase {
            root: group,
            templates: HashMap::new(),
        };

        let entries = db.get_entries(group_uuid, None);
        assert!(entries.len() > 0);
        assert_eq!(entries[0].uuid, entry_uuid);
    }

    #[test]
    fn search_entries_in_root_group() {
        let group_uuid =
            Uuid::from_str("133b7912-7705-4967-bc6e-807761ba9479").expect("bad group uuid");

        let entry_uuid1 =
            Uuid::from_str("d7e5dcb1-1e36-4b2a-9468-74f699809c1d").expect("bad entry uuid");

        let entry_uuid2 =
            Uuid::from_str("3c79f95c-842a-4e70-aed9-6cb7d128b01e").expect("bad entry uuid");

        let entry1 = RxEntry {
            uuid: entry_uuid1,
            title: Some(RxValue::try_from("test title".to_string()).expect("bad value")),
            ..Default::default()
        };

        let entry2 = RxEntry {
            uuid: entry_uuid2,
            title: Some(
                RxValue::try_from("should not show up".to_string()).expect("bad value"),
            ),
            ..Default::default()
        };

        let group = RxGroup {
            entries: vec![entry1, entry2],
            name: "asdf".to_string(),
            subgroups: vec![],
            uuid: group_uuid,
            parent: None,
        };

        let db = RxDatabase {
            root: group,
            templates: HashMap::new(),
        };

        let entries = db.get_entries(group_uuid, Some("test"));

        assert!(
            entries.len() == 1,
            "expected 1 entry, but got {}",
            entries.len()
        );
        assert_eq!(entries[0].uuid, entry_uuid1);
    }
}
