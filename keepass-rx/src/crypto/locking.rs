use std::fmt;
use std::ops::Deref;
use std::sync::OnceLock;

use libsodium_rs::utils::{mlock, munlock};
use rustix::process::{Resource, getrlimit};
use secstr::SecUtf8;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

use super::constant_time_eq;

const PROBE_BYTES: usize = 4096;
const MIN_LOCKABLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockPolicy {
    Everything,
    LongLivedOnly,
}

pub fn lock_policy() -> LockPolicy {
    static POLICY: OnceLock<LockPolicy> = OnceLock::new();
    *POLICY.get_or_init(probe)
}

fn probe() -> LockPolicy {
    let mut page = Zeroizing::new(vec![0u8; PROBE_BYTES]);
    let locked = mlock(&mut page).is_ok() && munlock(&mut page).is_ok();
    let roomy = getrlimit(Resource::Memlock)
        .current
        .is_none_or(|limit| limit >= MIN_LOCKABLE_BYTES);
    if locked && roomy {
        LockPolicy::Everything
    } else {
        LockPolicy::LongLivedOnly
    }
}

#[derive(Clone)]
pub enum SecretText {
    Locked(SecUtf8),
    Wiped(Zeroizing<String>),
}

impl SecretText {
    pub fn new(text: String) -> Self {
        match lock_policy() {
            LockPolicy::Everything => Self::Locked(SecUtf8::from(text)),
            LockPolicy::LongLivedOnly => Self::Wiped(Zeroizing::new(text)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Locked(text) => text.unsecure(),
            Self::Wiped(text) => text,
        }
    }
}

impl Default for SecretText {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl From<String> for SecretText {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for SecretText {
    fn from(text: &str) -> Self {
        Self::new(text.to_owned())
    }
}

impl Deref for SecretText {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for SecretText {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(self.as_str(), other.as_str())
    }
}

impl Eq for SecretText {}

impl fmt::Debug for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretText(***)")
    }
}

impl Serialize for SecretText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_and_round_trips() {
        let text = SecretText::from("hunter2");
        assert_eq!(text, SecretText::from("hunter2".to_string()));
        assert_ne!(text, SecretText::from("hunter3"));
        let json = serde_json::to_string(&text).expect("serialize");
        assert_eq!(json, "\"hunter2\"");
        let back: SecretText = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&*back, "hunter2");
        assert_eq!(format!("{text:?}"), "SecretText(***)");
    }
}
