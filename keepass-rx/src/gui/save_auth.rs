use std::future::Future;
use std::pin::Pin;

use anyhow::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAuthMethod {
    ShortCode,
    #[cfg(feature = "sailfish")]
    System,
    #[cfg(feature = "sailfish")]
    Password,
}

impl SaveAuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaveAuthMethod::ShortCode => "shortCode",
            #[cfg(feature = "sailfish")]
            SaveAuthMethod::System => "system",
            #[cfg(feature = "sailfish")]
            SaveAuthMethod::Password => "password",
        }
    }

    pub fn needs_unlock_key(&self) -> bool {
        match self {
            SaveAuthMethod::ShortCode => true,
            #[cfg(feature = "sailfish")]
            SaveAuthMethod::System => true,
            #[cfg(feature = "sailfish")]
            SaveAuthMethod::Password => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("System authorization is not available")]
pub struct SystemAuthUnavailable;

pub type Confirmation = Pin<Box<dyn Future<Output = Result<()>>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthPurpose {
    Save,
    Unlock,
}

pub trait SaveAuthorizer {
    fn method(&self) -> SaveAuthMethod;

    fn confirm(&self, _purpose: AuthPurpose) -> Option<Confirmation> {
        None
    }

    fn remember(&self) {}

    fn forget(&self) {}

    fn set_available(&self, _available: bool) {}
}

pub struct ShortCodeAuthorizer;

impl SaveAuthorizer for ShortCodeAuthorizer {
    fn method(&self) -> SaveAuthMethod {
        SaveAuthMethod::ShortCode
    }
}

#[cfg(feature = "sailfish")]
pub fn default_authorizer() -> Box<dyn SaveAuthorizer> {
    Box::new(super::polkit_auth::PolkitAuthorizer::default())
}

#[cfg(not(feature = "sailfish"))]
pub fn default_authorizer() -> Box<dyn SaveAuthorizer> {
    Box::new(ShortCodeAuthorizer)
}
