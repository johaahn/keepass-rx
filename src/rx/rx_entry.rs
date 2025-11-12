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
use std::mem;
use std::str::FromStr;
use totp_rs::{Secret, TOTP};
use uriparse::URI;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

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

    pub custom_fields: RxCustomFields,

    pub url: Option<RxValue>,
    pub raw_otp_value: Option<RxValue>,

    #[zeroize(skip)]
    pub icon: RxIcon,
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

        // Icon: Can eiher be the custom one (provided), or the
        // built-in one, or nothing.

        let rx_icon = icon
            .map(|i| RxIcon::Image(i.data))
            .or_else(|| entry.icon_id.map(|id| RxIcon::Builtin(id)))
            .unwrap_or(RxIcon::None);

        // Has to come after the above, otherwise those fields end up
        // in the custom fields.
        let mut remaining_fields = RxCustomFields::from(extract_remaining_fields(&mut entry));
        let mut custom_fields = RxCustomFields::from(custom_data);
        custom_fields.append(&mut remaining_fields);

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
            icon: rx_icon,
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
pub struct RxTotp {
    pub code: String,
    pub valid_for: String,
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
