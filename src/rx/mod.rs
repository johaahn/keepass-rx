use anyhow::{Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use humanize_duration::Truncate;
use humanize_duration::prelude::DurationExt;
use infer;
use keepass::db::{CustomData, Entry, Group, Icon, NodeRefMut, TOTP as KeePassTOTP, Value};
use qmetaobject::{QString, QVariant, QVariantMap};
use querystring::querify;
use secrecy::{ExposeSecret, SecretString};
use std::mem::take;
use std::{collections::HashMap, str::FromStr};
use totp_rs::{Secret, TOTP};
use uriparse::URI;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

mod easy_open;
mod zeroable_db;

pub use easy_open::EncryptedPassword;
pub use zeroable_db::ZeroableDatabase;

// Fields inserted by other KeePass programs that we do not want to
// show as custom fields. They might be used for other things in the
// app, though.
const FIELDS_TO_HIDE: [&str; 2] = ["KeePassXC-Browser Settings", "_LAST_MODIFIED"];

macro_rules! expose {
    ($secret:expr) => {{
        $secret
            .as_ref()
            .map(SecretString::expose_secret)
            .unwrap_or_default()
    }};
}

macro_rules! expose_opt {
    ($secret:expr) => {{
        $secret
            .as_ref()
            .map(|secret| secret.expose_secret().to_string())
    }};
}

pub(crate) use expose;

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
            name: take(&mut group.name),
            subgroups: subgroups,
            entries: entries,
            parent: None,
        }
    }
}

#[derive(Zeroize, Default, Clone)]
pub struct RxCustomFields(Vec<(String, SecretString)>);

impl From<&mut CustomData> for RxCustomFields {
    fn from(value: &mut CustomData) -> Self {
        let items = take(&mut value.items);

        let custom_fields: Vec<_> = items
            .into_iter()
            .flat_map(|(key, item)| {
                if !FIELDS_TO_HIDE.contains(&key.as_ref()) {
                    item.value.and_then(|value| match value {
                        Value::Protected(val) => {
                            Some((key, SecretString::from(val.to_string())))
                        }
                        Value::Unprotected(val) => Some((key, SecretString::from(val))),
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

impl From<RxCustomFields> for QVariantMap {
    fn from(value: RxCustomFields) -> QVariantMap {
        let mut map: HashMap<String, QVariant> = HashMap::new();

        for (key, value) in value.0 {
            map.insert(key, QString::from(value.expose_secret()).into());
        }

        map.into()
    }
}

impl Into<QVariant> for RxCustomFields {
    fn into(self) -> QVariant {
        Into::<QVariantMap>::into(self).into()
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

impl From<QString> for RxFieldName {
    fn from(value: QString) -> Self {
        RxFieldName::from(value.to_string())
    }
}

#[derive(Zeroize, Default, Clone)]
pub struct RxEntry {
    #[zeroize(skip)]
    pub uuid: Uuid,

    /// An entry always has a parent group.
    #[zeroize(skip)]
    pub parent_group: Uuid,

    pub title: Option<SecretString>,
    pub username: Option<SecretString>,
    pub password: Option<SecretString>,
    pub notes: Option<String>,

    // A map would be better, but it's not zeroizable.
    pub custom_fields: RxCustomFields,

    pub url: Option<SecretString>,
    pub raw_otp_value: Option<SecretString>,
    pub icon_data: Option<Vec<u8>>,
}

#[allow(dead_code)]
impl RxEntry {
    pub fn new_without_icon(entry: &mut Entry, parent_uuid: Uuid) -> Self {
        Self::new(entry, parent_uuid, None)
    }

    pub fn new(entry: &mut Entry, parent_uuid: Uuid, icon: Option<Icon>) -> Self {
        let notes = entry
            .fields
            .iter_mut()
            .find(|(key, _)| key.as_str() == "Notes")
            .and_then(|(_, mut value)| {
                match &mut value {
                    Value::Protected(_) => Some("[Hidden]".to_string()),
                    Value::Unprotected(val) => Some(take(val)),
                    _ => None, // discard binary notes? does that even exist?
                }
            });

        let mut custom_fields = take(&mut entry.custom_data);

        Self {
            uuid: entry.uuid,
            parent_group: parent_uuid,
            title: entry.get_title().map(SecretString::from),
            username: entry.get_username().map(SecretString::from),
            password: entry.get_password().map(SecretString::from),
            notes: notes,
            custom_fields: RxCustomFields::from(&mut custom_fields),
            url: entry.get_url().map(SecretString::from),
            raw_otp_value: entry.get_raw_otp_value().map(SecretString::from),
            icon_data: icon.map(|i| i.data),
        }
    }

    pub fn get_field_value(&self, field_name: &RxFieldName) -> Option<String> {
        match field_name {
            RxFieldName::Username => expose_opt!(self.username),
            RxFieldName::Password => expose_opt!(self.password),
            RxFieldName::Url => expose_opt!(self.url),
            RxFieldName::Title => expose_opt!(self.title),
            RxFieldName::CustomField(name) => {
                self.custom_fields.0.iter().find_map(|(key, value)| {
                    if key == name {
                        Some(value.expose_secret().to_owned())
                    } else {
                        None
                    }
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

        let raw_otp = expose!(self.raw_otp_value);
        let uri = URI::try_from(raw_otp)?;

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
        let otp = self
            .raw_otp_value
            .as_ref()
            .map(|otp| otp.expose_secret())
            .map(|value| KeePassTOTP::from_str(value))
            .ok_or(anyhow!("Unable to parse OTP"))??;

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

impl From<&RxEntry> for QVariantMap {
    fn from(value: &RxEntry) -> Self {
        let icon_data_url: QString =
            value.icon_data_url().map(QString::from).unwrap_or_default();

        let mapper = |value: &Option<SecretString>| {
            value
                .as_ref()
                .map(|secret| QString::from(secret.expose_secret()))
                .unwrap_or_default()
        };

        let totp = value.totp();

        let uuid: QString = value.uuid.to_string().into();
        let url: QString = mapper(&value.url);
        let username: QString = mapper(&value.username);
        let title: QString = mapper(&value.title);
        let password: QString = mapper(&value.password);
        let notes: QString = value.notes.clone().map(QString::from).unwrap_or_default();

        let mut map: HashMap<String, QVariant> = HashMap::new();
        map.insert("uuid".to_string(), uuid.into());
        map.insert("url".to_string(), url.into());
        map.insert("username".to_string(), username.into());
        map.insert("title".to_string(), title.into());
        map.insert("password".to_string(), password.into());
        map.insert("notes".to_string(), notes.into());
        map.insert("iconPath".to_string(), icon_data_url.into());
        map.insert(
            "customFields".to_string(),
            value.custom_fields.clone().into(),
        );

        if let Ok(_) = totp {
            map.insert("hasTotp".to_string(), true.into());
        } else {
            map.insert("hasTotp".to_string(), false.into());
        }

        map.into()
    }
}

impl From<RxEntry> for QVariantMap {
    fn from(value: RxEntry) -> QVariantMap {
        QVariantMap::from(&value)
    }
}

impl From<RxEntry> for QVariant {
    fn from(value: RxEntry) -> Self {
        Into::<QVariantMap>::into(value).into()
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxDatabase {
    root: RxGroup,
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

fn load_groups_recursive(group: &mut Group, icons: &mut HashMap<Uuid, Icon>) -> Vec<RxGroup> {
    let this_group_id = group.uuid;
    let mut groups = vec![];
    let mut subgroups = vec![];
    let mut entries = vec![];

    for node in group.children.iter_mut() {
        match node.as_mut() {
            NodeRefMut::Group(mut subgroup) => {
                let mut rx_groups: Vec<_> = load_groups_recursive(&mut subgroup, icons)
                    .into_iter()
                    .map(|mut subgroup| {
                        subgroup.parent = Some(this_group_id);
                        subgroup
                    })
                    .collect();

                subgroups.append(&mut rx_groups);
            }
            NodeRefMut::Entry(entry) => {
                let icon = entry
                    .custom_icon_uuid
                    .and_then(|icon_uuid| icons.remove(&icon_uuid));
                entries.push(RxEntry::new(entry, group.uuid, icon));
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
    pub fn new(db: &mut Zeroizing<ZeroableDatabase>) -> Self {
        let mut icons: HashMap<Uuid, Icon> = db
            .meta
            .custom_icons
            .icons
            .iter()
            .map(|icon| (icon.uuid, icon.to_owned()))
            .collect();

        // There should only be one group in the vec, which is the
        // root.
        let mut rx_groups = load_groups_recursive(&mut db.root, &mut icons);
        let root_group = rx_groups.swap_remove(0);

        Self { root: root_group }
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

    pub fn get_entries(&self, group_uuid: Uuid, search_term: Option<&str>) -> Vec<&RxEntry> {
        let search_term = search_term.map(|term| term.to_lowercase());
        let group = self.get_group(group_uuid);
        let entries_in_group = group.as_ref().map(|group| group.entries.as_slice());

        // Determine if an entry should show up in search results, if
        // a search term was specified. term is already lowecase here.
        let search_entry = |entry: &RxEntry, term: &str| {
            let username = expose!(entry.username).to_lowercase();
            let url = expose!(entry.url).to_lowercase();
            let title = expose!(entry.title).to_lowercase();
            username.contains(term) || url.contains(term) || title.contains(term)
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

        let db = RxDatabase { root: group };
        let entries = db.get_entries(group_uuid, None);
        assert!(entries.len() > 0);
        assert_eq!(entries[0].uuid, entry_uuid);
    }
}
