use std::cell::{Cell, OnceCell};
use std::collections::HashMap;

use anyhow::{Result, anyhow};
use log::warn;
use qttypes::QSettings;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use super::save_auth::{
    AuthPurpose, Confirmation, SaveAuthMethod, SaveAuthorizer, SystemAuthUnavailable,
};
use crate::crypto::EncryptedPassword;

const AUTHORITY_DESTINATION: &str = "org.freedesktop.PolicyKit1";
const AUTHORITY_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const AUTHORITY_INTERFACE: &str = "org.freedesktop.PolicyKit1.Authority";
const CHECK_AUTHORIZATION: &str = "CheckAuthorization";
const ALLOW_USER_INTERACTION: u32 = 1;
const DEVICE_LOCK_SETTINGS: &str = "/usr/share/lipstick/devicelock/devicelock_settings.conf";
const CODE_LENGTH_KEY: &str = "desktop/nemo/devicelock/code_current_length";

type Subject<'a> = (&'a str, HashMap<&'a str, Value<'a>>);
type AuthorizationResult = (bool, bool, HashMap<String, String>);

pub fn lock_code_missing() -> bool {
    let settings = QSettings::from_path(DEVICE_LOCK_SETTINGS);
    settings.contains(CODE_LENGTH_KEY) && settings.value_string(CODE_LENGTH_KEY) == "0"
}

fn action_id(purpose: AuthPurpose) -> &'static str {
    match purpose {
        AuthPurpose::Save => "harbour.keepassrx.save-database",
        AuthPurpose::Unlock => "harbour.keepassrx.unlock-database",
    }
}

fn check_authorization(purpose: AuthPurpose) -> Result<()> {
    let connection = Connection::system()?;
    let unique_name = connection
        .unique_name()
        .ok_or_else(|| anyhow!("The system bus gave no unique name"))?
        .to_string();

    let subject: Subject = (
        "system-bus-name",
        HashMap::from([("name", Value::from(unique_name))]),
    );
    let details: HashMap<&str, &str> = HashMap::new();

    let reply = connection.call_method(
        Some(AUTHORITY_DESTINATION),
        AUTHORITY_PATH,
        Some(AUTHORITY_INTERFACE),
        CHECK_AUTHORIZATION,
        &(
            subject,
            action_id(purpose),
            details,
            ALLOW_USER_INTERACTION,
            "",
        ),
    )?;
    let (authorized, _, _): AuthorizationResult = reply.body().deserialize()?;

    if authorized {
        Ok(())
    } else {
        Err(anyhow!("The system did not authorize {purpose:?}"))
    }
}

#[derive(Default)]
pub struct PolkitAuthorizer {
    stored: Cell<bool>,
    available: OnceCell<bool>,
}

impl SaveAuthorizer for PolkitAuthorizer {
    fn method(&self) -> SaveAuthMethod {
        if self.available.get() == Some(&true) {
            SaveAuthMethod::System
        } else {
            SaveAuthMethod::Password
        }
    }

    fn confirm(&self, purpose: AuthPurpose) -> Option<Confirmation> {
        if !self.stored.get() || self.available.get() != Some(&true) {
            return Some(Box::pin(async { Err(SystemAuthUnavailable.into()) }));
        }

        Some(Box::pin(async move {
            actix_rt::task::spawn_blocking(move || check_authorization(purpose))
                .await
                .map_err(|err| anyhow!(err))?
        }))
    }

    fn remember(&self) {
        self.stored.set(true);
    }

    fn forget(&self) {
        self.stored.set(false);
        EncryptedPassword::forget_saved();
    }

    fn set_available(&self, available: bool) {
        if self.available.set(available).is_err() {
            warn!("System unlock availability was already set");
        }
    }
}
