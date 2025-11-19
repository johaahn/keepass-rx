use crate::crypto::{DefaultWithKey, EncryptedValue, MasterKey};

use super::icons::RxIcon;
use anyhow::{Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use humanize_duration::Truncate;
use humanize_duration::prelude::DurationExt;
use infer;
use keepass::db::{CustomData, Entry, Icon, TOTP as KeePassTOTP, Value};
use libsodium_rs::utils::{SecureVec, vec_utils};
use querystring::querify;
use secstr::SecStr;
use std::borrow::Cow;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::{mem, sync::atomic::AtomicU64};
use totp_rs::{Secret, TOTP};
use uriparse::URI;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

macro_rules! expose {
    ($masterkey:expr, $secret:expr) => {{
        $secret
            .as_ref()
            .and_then(|secret| secret.value($masterkey).map(|value| value.to_string()))
            .unwrap_or_default()
    }};
}

macro_rules! expose_str {
    ($masterkey:expr, $secret:expr) => {{
        $secret
            .as_ref()
            .and_then(|secret| secret.value($masterkey))
            .unwrap_or_default()
    }};
}

pub(crate) use {expose, expose_str};

// Special field that indicates the entry is a templated entry (e.g.
// credit card, wifi password, etc).
pub(crate) const TEMPLATE_FIELD_NAME: &str = "_etm_template_uuid";

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
const WILDCARD_FIELDS_TO_HIDE: [&str; 3] = ["AndroidApp", "KP2A_URL", "KPEX_PASSKEY_"];

fn should_hide_field(field_name: &str) -> bool {
    FIELDS_TO_HIDE.contains(&field_name)
        || WILDCARD_FIELDS_TO_HIDE
            .iter()
            .any(|wildcard| field_name.starts_with(wildcard))
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct RxEntry {
    #[zeroize(skip)]
    pub uuid: Uuid,

    #[zeroize(skip)]
    pub(super) master_key: Rc<MasterKey>,

    /// An entry always has a parent group.
    #[zeroize(skip)]
    pub parent_group: Uuid,

    #[zeroize(skip)]
    pub template_uuid: Option<Uuid>,

    pub(super) title: Option<RxValue>,
    pub(super) username: Option<RxValue>,
    pub(super) password: Option<RxValue>,
    pub(super) notes: Option<RxValue>,
    pub(super) tags: Vec<String>,

    pub custom_fields: RxCustomFields,

    pub(super) url: Option<RxValue>,
    pub(super) raw_otp_value: Option<RxValue>,

    #[zeroize(skip)]
    pub icon: RxIcon,
}

fn extract_remaining_fields(
    master_key: &MasterKey,
    entry: &mut Entry,
) -> Vec<(String, RxValue)> {
    entry
        .fields
        .drain()
        .flat_map(|(key, value)| {
            match value {
                Value::Protected(val) => RxValue::encrypted(master_key, val)
                    .ok()
                    .map(|rx_val| (key, rx_val)),
                Value::Unprotected(val) => {
                    RxValue::try_from(val).ok().map(|rx_val| (key, rx_val))
                }
                _ => None, // discard binary for now. attachments later?
            }
        })
        .collect()
}

fn extract_value(
    master_key: &MasterKey,
    entry: &mut Entry,
    field_name: &str,
) -> Option<RxValue> {
    entry.fields.remove(field_name).and_then(|value| {
        match value {
            Value::Protected(val) => RxValue::encrypted(master_key, val).ok(),
            Value::Unprotected(val) => RxValue::try_from(val).ok(),
            _ => None, // discard binary for now. attachments later?
        }
    })
}

impl DefaultWithKey for RxCustomFields {
    fn default_with_key(key: &Rc<MasterKey>) -> Self {
        Self {
            master_key: key.clone(),
            data: Default::default(),
        }
    }
}

impl DefaultWithKey for RxEntry {
    fn default_with_key(key: &Rc<MasterKey>) -> Self {
        Self {
            master_key: key.clone(),
            custom_fields: DefaultWithKey::default_with_key(key),
            icon: Default::default(),
            notes: Default::default(),
            parent_group: Default::default(),
            password: Default::default(),
            raw_otp_value: Default::default(),
            template_uuid: Default::default(),
            title: Default::default(),
            url: Default::default(),
            username: Default::default(),
            uuid: Default::default(),
            tags: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl RxEntry {
    pub fn new(
        master_key: &Rc<MasterKey>,
        mut entry: Entry,
        parent_uuid: Uuid,
        icon: Option<Icon>,
    ) -> Self {
        let master_key = master_key.clone();

        let custom_data = mem::take(&mut entry.custom_data);

        let title = extract_value(&master_key, &mut entry, "Title");
        let username = extract_value(&master_key, &mut entry, "UserName");
        let password = extract_value(&master_key, &mut entry, "Password");
        let notes = extract_value(&master_key, &mut entry, "Notes");
        let url = extract_value(&master_key, &mut entry, "URL");
        let raw_otp_value = extract_value(&master_key, &mut entry, "otp");

        // Template: Extract the _etm_template_uuid field, which
        // points to a template DB entry.
        let template_uuid = extract_value(&master_key, &mut entry, TEMPLATE_FIELD_NAME)
            .and_then(|val| {
                val.value(&master_key)
                    .and_then(|uuid_str| Uuid::from_str(&uuid_str).ok())
            });

        // Icon: Can eiher be the custom one (provided), or the
        // built-in one, or nothing.
        let rx_icon = icon
            .map(|i| RxIcon::Image(i.data))
            .or_else(|| entry.icon_id.map(|id| RxIcon::Builtin(id)))
            .unwrap_or(RxIcon::None);

        // Has to come after the above, otherwise those fields end up
        // in the custom fields.
        let remaining_fields = extract_remaining_fields(&master_key, &mut entry);
        let mut remaining_fields = RxCustomFields::from_vec(&master_key, remaining_fields);
        let mut custom_fields = RxCustomFields::from_custom_data(&master_key, custom_data);
        custom_fields.append(&mut remaining_fields);

        Self {
            uuid: entry.uuid,
            master_key: master_key,
            parent_group: parent_uuid,
            template_uuid: template_uuid,
            title: title,
            username: username,
            password: password,
            notes: notes,
            custom_fields: custom_fields,
            url: url,
            raw_otp_value: raw_otp_value,
            icon: rx_icon,
            tags: mem::take(&mut entry.tags),
        }
    }

    pub fn username(&self) -> Option<RxValueKeyRef<'_>> {
        self.username
            .as_ref()
            .map(|v| RxValueKeyRef::new(v, &self.master_key))
    }

    pub fn password(&self) -> Option<RxValueKeyRef<'_>> {
        self.password
            .as_ref()
            .map(|p| RxValueKeyRef::new(p, &self.master_key))
    }

    pub fn title(&self) -> Option<RxValueKeyRef<'_>> {
        self.title
            .as_ref()
            .map(|t| RxValueKeyRef::new(t, &self.master_key))
    }

    pub fn url(&self) -> Option<RxValueKeyRef<'_>> {
        self.url
            .as_ref()
            .map(|u| RxValueKeyRef::new(u, &self.master_key))
    }

    pub fn raw_otp_value(&self) -> Option<RxValueKeyRef<'_>> {
        self.raw_otp_value
            .as_ref()
            .map(|t| RxValueKeyRef::new(t, &self.master_key))
    }

    pub(super) fn master_key(&self) -> &MasterKey {
        &self.master_key
    }

    pub fn get_field_value(&self, field_name: &RxFieldName) -> Option<RxValueKeyRef<'_>> {
        match field_name {
            RxFieldName::Username => self
                .username
                .as_ref()
                .map(|val| RxValueKeyRef::new(val, &self.master_key)),
            RxFieldName::Password => self
                .password
                .as_ref()
                .map(|val| RxValueKeyRef::new(val, &self.master_key)),
            RxFieldName::Url => self
                .url
                .as_ref()
                .map(|val| RxValueKeyRef::new(val, &self.master_key)),
            RxFieldName::Title => self
                .title
                .as_ref()
                .map(|val| RxValueKeyRef::new(val, &self.master_key)),
            RxFieldName::CurrentTotp => self
                .totp()
                .ok()
                .map(|val| RxValueKeyRef::new(RxValue::CurrentTotp(val), &self.master_key)),
            RxFieldName::CustomField(name) => {
                self.custom_fields
                    .data
                    .iter()
                    .find_map(|(key, value)| match key == name {
                        true => Some(RxValueKeyRef::new(value, &self.master_key)),
                        false => None,
                    })
            }
        }
    }

    pub fn has_tags(&self) -> bool {
        self.tags.len() > 0
    }

    pub fn has_otp(&self) -> bool {
        self.raw_otp_value.is_some()
    }

    pub fn has_steam_otp(&self) -> bool {
        expose!(&self.master_key, self.raw_otp_value).starts_with("otpauth://totp/Steam:")
    }

    pub fn steam_otp_digits(&self) -> Result<String> {
        if !self.has_steam_otp() {
            return Err(anyhow!("Not a Steam OTP entry"));
        }

        let raw_otp = expose_str!(&self.master_key, self.raw_otp_value);
        let uri = URI::try_from(raw_otp.as_str())?;

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
        let otp =
            KeePassTOTP::from_str(expose_str!(&self.master_key, self.raw_otp_value).as_str())?;

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
        if let RxIcon::Image(ref data) = self.icon {
            infer::get(data).map(|k| {
                format!(
                    "data:{};base64,{}",
                    k.mime_type(),
                    BASE64_STANDARD.encode(data)
                )
            })
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct RxValueKeyRef<'a>(Cow<'a, RxValue>, &'a MasterKey);

impl<'a> RxValueKeyRef<'a> {
    pub fn new<V>(value: V, key: &'a MasterKey) -> Self
    where
        V: Into<Cow<'a, RxValue>>,
    {
        Self(value.into(), key)
    }

    pub fn value(&self) -> Option<Zeroizing<String>> {
        self.0.value(self.1).map(Zeroizing::new)
    }

    pub fn is_hidden_by_default(&self) -> bool {
        self.0.is_hidden_by_default()
    }

    pub fn totp_value(&self) -> Option<&RxTotp> {
        self.0.totp_value()
    }
}

impl<'a> From<RxValue> for Cow<'a, RxValue> {
    fn from(value: RxValue) -> Self {
        Cow::Owned(value)
    }
}

impl<'a> From<&'a RxValue> for Cow<'a, RxValue> {
    fn from(value: &'a RxValue) -> Self {
        Cow::Borrowed(value)
    }
}

/// Not Sync or Send, because of SecureVec using *mut u8.
#[derive(Default, ZeroizeOnDrop, Clone)]
pub enum RxValue {
    /// Fully hidden by default. Used for passwords etc.
    Protected(EncryptedValue),

    /// Not hidden by default, but we don't want to leak the metadata
    /// in memory if possible.
    Sensitive(SecureVec<u8>),

    /// Regular value not hidden or treated specially by memory
    /// protection.
    Unprotected(String),

    /// Unprotected, used when generating a TOTP value for the given moment.
    CurrentTotp(RxTotp),

    #[default]
    Unsupported,
}

impl Zeroize for RxValue {
    fn zeroize(&mut self) {
        match self {
            RxValue::Protected(encrypted_value) => encrypted_value.zeroize(),
            RxValue::Sensitive(val) => val.zeroize(),
            RxValue::Unprotected(val) => val.zeroize(),
            _ => (),
        }
    }
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl RxValue {
    pub fn encrypted(master_key: &MasterKey, mut value: SecStr) -> Result<Self> {
        let value_unsecure = value.unsecure();
        let mut secure_vec = vec_utils::secure_vec::<u8>(value_unsecure.len())?;
        secure_vec.copy_from_slice(&value_unsecure);
        value.zero_out();

        let encrypted_value = EncryptedValue::new(
            master_key,
            ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            secure_vec,
        )?;

        Ok(Self::Protected(encrypted_value))
    }

    pub fn is_hidden_by_default(&self) -> bool {
        match self {
            RxValue::Protected(_) => true,
            _ => false,
        }
    }

    pub fn value(&self, master_key: &MasterKey) -> Option<String> {
        use RxValue::*;
        match self {
            Protected(val) => val
                .expose(master_key)
                .map(|val| std::str::from_utf8(&val).ok().map(|v| v.to_string()))
                .ok()
                .flatten(),
            Sensitive(val) => std::str::from_utf8(&val).ok().map(|v| v.to_string()),
            Unprotected(value) => Some(value.clone()),
            _ => None,
        }
    }

    pub fn totp_value(&self) -> Option<&RxTotp> {
        match self {
            RxValue::CurrentTotp(totp) => Some(totp),
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

// impl TryFrom<SecStr> for RxValue {
//     type Error = anyhow::Error;
//     fn try_from(mut value: SecStr) -> std::result::Result<Self, Self::Error> {
//         let value_unsecure = value.unsecure();
//         let mut secure_vec = vec_utils::secure_vec::<u8>(value_unsecure.len())?;
//         secure_vec.copy_from_slice(&value_unsecure);
//         value.zero_out();
//         Ok(RxValue::Protected(secure_vec))
//     }
// }

impl TryFrom<String> for RxValue {
    type Error = anyhow::Error;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let mut secure_vec = vec_utils::secure_vec::<u8>(value.len())?;
        secure_vec.copy_from_slice(value.as_ref());
        Ok(RxValue::Sensitive(secure_vec))
    }
}

#[derive(Zeroize, Default, Clone)]
pub struct RxTotp {
    pub code: String,
    pub valid_for: String,
}

#[derive(Clone, PartialEq)]
pub enum RxFieldName {
    Title,
    Username,
    Password,
    Url,
    CurrentTotp,
    CustomField(String),
}

impl ToString for RxFieldName {
    fn to_string(&self) -> String {
        match self {
            RxFieldName::Username => "Username".to_string(),
            RxFieldName::Password => "Password".to_string(),
            RxFieldName::Title => "Title".to_string(),
            RxFieldName::Url => "URL".to_string(),
            RxFieldName::CurrentTotp => "CurrentTOTP".to_string(),
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
            "currenttotp" => RxFieldName::CurrentTotp,
            _ => RxFieldName::CustomField(value),
        }
    }
}

#[derive(Zeroize, ZeroizeOnDrop, Clone)]
pub struct RxCustomFields {
    #[zeroize(skip)]
    master_key: Rc<MasterKey>,
    data: Vec<(String, RxValue)>,
}

impl RxCustomFields {
    fn from_vec(master_key: &Rc<MasterKey>, value: Vec<(String, RxValue)>) -> Self {
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

        Self {
            master_key: master_key.clone(),
            data: custom_fields,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, RxValueKeyRef<'_>)> {
        self.data
            .iter()
            .map(|(key, value)| (key, RxValueKeyRef::new(value, &self.master_key)))
    }

    fn from_custom_data(master_key: &Rc<MasterKey>, value: CustomData) -> Self {
        let custom_fields: Vec<_> = value
            .items
            .into_iter()
            .flat_map(|(key, item)| {
                if !should_hide_field(&key.as_ref()) {
                    item.value.and_then(|value| match value {
                        Value::Protected(val) => RxValue::encrypted(master_key, val)
                            .ok()
                            .map(|secret| (key, secret)),
                        Value::Unprotected(val) => {
                            RxValue::try_from(val).ok().map(|secret| (key, secret))
                        }
                        _ => None,
                    })
                } else {
                    None
                }
            })
            .collect();

        Self {
            master_key: master_key.clone(),
            data: custom_fields,
        }
    }

    pub fn append(&mut self, other: &mut RxCustomFields) {
        self.data.append(&mut other.data);
    }
}
