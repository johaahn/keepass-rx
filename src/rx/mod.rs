use anyhow::{Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use humanize_duration::Truncate;
use humanize_duration::prelude::DurationExt;
use infer;
use keepass::db::{CustomData, Entry, Group, Icon, NodeRef, TOTP as KeePassTOTP, Value};
use qmetaobject::{QString, QVariant, QVariantMap};
use querystring::querify;
use secrecy::{ExposeSecret, SecretString};
use std::{collections::HashMap, ops::Deref, str::FromStr};
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

#[derive(Zeroize, Default, Clone)]
pub struct RxTotp {
    pub code: String,
    pub valid_for: String,
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxGroup {
    #[zeroize(skip)]
    pub uuid: Uuid,
    pub name: String,
    pub entries: Vec<RxEntry>,
}

impl RxGroup {
    pub fn new(group: &Group, entries: Vec<RxEntry>) -> Self {
        Self {
            uuid: group.uuid,
            name: group.name.clone(),
            entries: entries,
        }
    }
}

#[derive(Zeroize, Default, Clone)]
struct RxCustomFields(Vec<(String, SecretString)>);

impl From<CustomData> for RxCustomFields {
    fn from(value: CustomData) -> Self {
        let custom_fields: Vec<_> = value
            .items
            .iter()
            .flat_map(|(key, item)| {
                if !FIELDS_TO_HIDE.contains(&key.as_ref()) {
                    item.value.clone().and_then(|value| match value {
                        Value::Protected(val) => {
                            Some((key.clone(), SecretString::from(val.to_string())))
                        }
                        Value::Unprotected(val) => {
                            Some((key.clone(), SecretString::from(val)))
                        }
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

impl Into<QVariantMap> for RxCustomFields {
    fn into(self) -> QVariantMap {
        let mut map: HashMap<String, QVariant> = HashMap::new();

        for (key, value) in self.0 {
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

#[derive(Zeroize, Default, Clone)]
pub struct RxEntry {
    #[zeroize(skip)]
    uuid: Uuid,

    title: Option<SecretString>,
    username: Option<SecretString>,
    password: Option<SecretString>,
    notes: Option<String>,

    // A map would be better, but it's not zeroizable.
    custom_fields: RxCustomFields,

    url: Option<SecretString>,
    raw_otp_value: Option<SecretString>,
    icon_data: Option<Vec<u8>>,
}

#[allow(dead_code)]
impl RxEntry {
    pub fn new_without_icon(entry: Entry) -> Self {
        Self::new(entry, None)
    }

    pub fn new(entry: Entry, icon: Option<Icon>) -> Self {
        let notes = entry
            .fields
            .iter()
            .find(|(key, _)| key.as_str() == "Notes")
            .and_then(|(_, value)| {
                match value {
                    Value::Protected(_) => Some("[Hidden]".to_string()),
                    Value::Unprotected(val) => Some(val.clone()),
                    _ => None, // discard binary notes? does that even exist?
                }
            });

        let custom_fields = entry.custom_data.clone();

        Self {
            uuid: entry.uuid,
            title: entry.get_title().map(SecretString::from),
            username: entry.get_username().map(SecretString::from),
            password: entry.get_password().map(SecretString::from),
            notes: notes,
            custom_fields: custom_fields.into(),
            url: entry.get_url().map(SecretString::from),
            raw_otp_value: entry.get_raw_otp_value().map(SecretString::from),
            icon_data: icon.map(|i| i.data),
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
            false => otp_code.code.clone(),
        };

        let otp_valid_for = format!("{}", otp_code.valid_for.human(Truncate::Second));

        Ok(RxTotp {
            code: otp_digits,
            valid_for: otp_valid_for,
        })
    }
}

impl Into<QVariantMap> for RxEntry {
    fn into(self) -> QVariantMap {
        // TODO support standard KeePass icons? (Icon IDs)
        let icon_data_url: QString = self
            .icon_data
            .as_deref()
            .and_then(|data| {
                // create qstring here so it will be null after unwrap
                // if no icon.
                infer::get(data).map(|k| {
                    QString::from(format!(
                        "data:{};base64,{}",
                        k.mime_type(),
                        BASE64_STANDARD.encode(data)
                    ))
                })
            })
            .unwrap_or_default();

        let mapper = |value: &Option<SecretString>| {
            value
                .as_ref()
                .map(|secret| QString::from(secret.expose_secret()))
                .unwrap_or_default()
        };

        let totp = self.totp();

        let uuid: QString = self.uuid.to_string().into();
        let url: QString = mapper(&self.url);
        let username: QString = mapper(&self.username);
        let title: QString = mapper(&self.title);
        let password: QString = mapper(&self.password);
        let notes: QString = self.notes.map(QString::from).unwrap_or_default();

        let mut map: HashMap<String, QVariant> = HashMap::new();
        map.insert("uuid".to_string(), uuid.into());
        map.insert("url".to_string(), url.into());
        map.insert("username".to_string(), username.into());
        map.insert("title".to_string(), title.into());
        map.insert("password".to_string(), password.into());
        map.insert("notes".to_string(), notes.into());
        map.insert("iconPath".to_string(), icon_data_url.into());
        map.insert("customFields".to_string(), self.custom_fields.into());

        if let Ok(_) = totp {
            map.insert("hasTotp".to_string(), true.into());
        } else {
            map.insert("hasTotp".to_string(), false.into());
        }

        map.into()
    }
}

impl Into<QVariant> for RxEntry {
    fn into(self) -> QVariant {
        Into::<QVariantMap>::into(self).into()
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Default, Clone)]
pub struct RxDatabase {
    groups: Vec<RxGroup>,
}

// Manual impl because otherwise printing debug will dump the raw
// contents of the ENTIRE database and crash the terminal.
impl std::fmt::Debug for RxDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RxDatabase")
            .field("groups", &self.groups.len())
            .field(
                "entries",
                &self
                    .groups
                    .iter()
                    .map(|group| group.entries.len())
                    .sum::<usize>(),
            )
            .finish()
    }
}

fn load_groups_recursive(group: &Group, icons: &HashMap<Uuid, Icon>) -> Vec<RxGroup> {
    let load_entry = |entry: &Entry| {
        let icon = entry
            .custom_icon_uuid
            .and_then(|icon_uuid| icons.get(&icon_uuid));

        RxEntry::new(entry.clone(), icon.cloned())
    };

    let mut groups = vec![];
    let mut entries = vec![];

    for node in group.children.iter() {
        match node.as_ref() {
            NodeRef::Group(group) => {
                let mut rx_groups = load_groups_recursive(group, icons);
                groups.append(&mut rx_groups);
            }
            NodeRef::Entry(entry) => {
                entries.push(load_entry(entry));
            }
        }
    }

    // Currently this will hide groups with no entries. Revisit
    // after new layout/once editing is enabled.
    if entries.len() > 0 {
        groups.push(RxGroup::new(&group, entries));
    }
    groups
}

impl RxDatabase {
    pub fn new(db: Zeroizing<ZeroableDatabase>) -> Self {
        let icons: HashMap<Uuid, Icon> = db
            .meta
            .custom_icons
            .icons
            .iter()
            .map(|icon| (icon.uuid, icon.to_owned()))
            .collect();

        let rx_groups = load_groups_recursive(&db.root, &icons);

        Self { groups: rx_groups }
    }

    pub fn close(&mut self) {
        println!("Closing database.");
        self.zeroize();
    }

    pub fn groups(&self) -> &[RxGroup] {
        self.groups.as_slice()
    }

    pub fn get_entries(&self, search_term: Option<&str>) -> HashMap<String, Vec<RxEntry>> {
        let search_term = search_term.map(|term| term.to_lowercase());

        let groups_and_entries = self
            .groups
            .iter()
            .map(|group| (group.clone(), group.entries.as_slice()));

        // Determine if an entry should show up in search results, if
        // a search term was specified. term is already lowecase here.
        let search_entry = |entry: &RxEntry, term: &str| {
            let username = expose!(entry.username).to_lowercase();
            let url = expose!(entry.url).to_lowercase();
            let title = expose!(entry.title).to_lowercase();
            username.contains(term) || url.contains(term) || title.contains(term)
        };

        let filtered_by_search = groups_and_entries.map(|(group, entries)| {
            let filtered_entries = entries
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
                .collect::<Vec<_>>();

            (group, filtered_entries)
        });

        // map of: group name -> list of entries
        let group_to_entry_list = filtered_by_search.fold(
            HashMap::<String, Vec<RxEntry>>::new(),
            |mut acc, (group, entries)| {
                let group_name = group.name.clone();
                let values = acc.entry(group_name).or_insert_with(|| vec![]);

                for entry in entries {
                    values.push(entry.clone());
                }
                acc
            },
        );

        group_to_entry_list
    }

    pub fn get_totp(&self, entry_uuid: &str) -> Result<RxTotp> {
        let entry_uuid = Uuid::from_str(entry_uuid)?;
        let entry = self
            .groups
            .iter()
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
