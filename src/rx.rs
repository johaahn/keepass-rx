use anyhow::{Result, anyhow};
use humanize_duration::Truncate;
use humanize_duration::prelude::DurationExt;
use keepass::{
    Database,
    db::{Entry, Group, Meta},
};
use qmetaobject::{QString, QVariant, QVariantMap};
use querystring::querify;
use std::{collections::HashMap, str::FromStr};
use totp_rs::{Secret, TOTP};
use uriparse::URI;
use uuid::Uuid;

pub struct RxTotp {
    pub code: String,
    pub valid_for: String,
}

pub struct RxEntry(Entry);

#[allow(dead_code)]
impl RxEntry {
    pub fn new_without_icons(entry: Entry) -> Self {
        Self(entry)
    }

    pub fn new(entry: Entry) -> Self {
        Self(entry)
    }

    pub fn has_steam_otp(&self) -> bool {
        let raw_otp = self.0.get_raw_otp_value().unwrap_or_default();
        raw_otp.starts_with("otpauth://totp/Steam:")
    }

    pub fn steam_otp_digits(&self) -> Result<String> {
        if !self.has_steam_otp() {
            return Err(anyhow!("Not a Steam OTP entry"));
        }

        let raw_otp = self.0.get_raw_otp_value().unwrap_or_default();
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
        let otp = self.0.get_otp()?;

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
        // TODO make use of icon
        // let _icon = self
        //     .0
        //     .custom_icon_uuid
        //     .and_then(|id| self.1.iter().find(|icon| icon.uuid == id));

        let mapper = |text: &str| QString::from(text);

        let uuid: QString = self.0.uuid.to_string().into();
        let url: QString = self.0.get_url().map(mapper).unwrap_or_default();
        let username: QString = self.0.get_username().map(mapper).unwrap_or_default();
        let title: QString = self.0.get_title().map(mapper).unwrap_or_default();
        let password: QString = self.0.get_password().map(mapper).unwrap_or_default();

        let mut map: HashMap<String, QVariant> = HashMap::new();
        map.insert("uuid".to_string(), uuid.into());
        map.insert("url".to_string(), url.into());
        map.insert("username".to_string(), username.into());
        map.insert("title".to_string(), title.into());
        map.insert("password".to_string(), password.into());
        map.insert("iconPath".to_string(), QString::from("").into());

        let totp = self.totp();

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

#[derive(Default, Clone)]
pub struct RxDatabase {
    db: Option<Database>,
    groups: Vec<Group>,
    entries: Vec<Entry>,
    meta: Meta,
}

// Manual impl because otherwise printing debug will dump the raw
// contents of the ENTIRE database and crash the terminal.
impl std::fmt::Debug for RxDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RxDatabase")
            .field(
                "db",
                match self.db {
                    Some(_) => &"loaded",
                    None => &"not loaded",
                },
            )
            .field("groups", &self.groups.len())
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl RxDatabase {
    pub fn new(db: Database) -> Self {
        Self {
            db: Some(db),
            ..Default::default()
        }
    }

    fn db(&self) -> &Database {
        // rc as_ref -> option as_ref
        self.db.as_ref().as_ref().expect("No database")
    }

    pub fn groups(&self) -> &[Group] {
        self.groups.as_slice()
    }

    pub fn load_data(&mut self) {
        self.meta = self.db().meta.clone();
        self.groups = self.db().root.groups().into_iter().cloned().collect();

        self.entries = self
            .db()
            .root
            .groups()
            .into_iter()
            .flat_map(|group| group.entries())
            .cloned()
            .collect();
    }

    pub fn get_entries(&self, search_term: Option<&str>) -> HashMap<String, Vec<RxEntry>> {
        let search_term = search_term.map(|term| term.to_lowercase());
        //let custom_icons = self.meta.custom_icons.icons.as_slice(); // TODO

        let groups_and_entries = self.groups.iter().map(|group| {
            (
                group.clone(),
                group.entries().into_iter().cloned().collect::<Vec<_>>(),
            )
        });

        // Determine if an entry should show up in search results, if
        // a search term was specified. term is already lowecase here.
        let search_entry = |entry: &Entry, term: &str| {
            let username = entry.get_username().unwrap_or_default().to_lowercase();
            let url = entry.get_url().unwrap_or_default().to_lowercase();
            let title = entry.get_title().unwrap_or_default().to_lowercase();
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
                .map(|entry| RxEntry(entry))
                .collect::<Vec<_>>();

            (group, filtered_entries)
        });

        // map of: group name -> list of entries
        let group_to_entry_list = filtered_by_search.fold(
            HashMap::<String, Vec<RxEntry>>::new(),
            |mut acc, (group, entries)| {
                let group_name = group.name.clone();
                let values = acc.entry(group_name).or_insert_with(|| vec![]);

                // converts into qvariant
                for entry in entries {
                    values.push(entry);
                }
                acc
            },
        );

        group_to_entry_list
    }

    pub fn get_totp(&self, entry_uuid: &str) -> Result<RxTotp> {
        let entry_uuid = Uuid::from_str(entry_uuid)?;
        let entry = self
            .entries
            .iter()
            .find(|&entry| entry.uuid == entry_uuid)
            .cloned();

        let otp = entry
            .map(|ent| RxEntry::new_without_icons(ent))
            .map(|kp_ent| kp_ent.totp())
            .transpose();

        match otp {
            Ok(Some(otp)) => Ok(otp),
            Ok(None) => Err(anyhow!("Could not find OTP entry")),
            Err(err) => Err(err),
        }
    }
}
