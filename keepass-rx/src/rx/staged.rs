use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::mem::take;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use atomic_write_file::AtomicWriteFile;
use base64::{Engine, prelude::BASE64_STANDARD};
use keepass::config::{DatabaseConfig, DatabaseVersion};
use keepass::db::fields;
use keepass::error::{DatabaseKeyError, DatabaseOpenError, DatabaseSaveError};
use keepass::{Database, DatabaseKey};
use libsodium_rs::crypto_secretbox::{self, NONCEBYTES, Nonce};
use log::warn;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::rx_edit::{RxEdit, RxEntryEdit, RxFieldEdit};
use super::rx_edit_values::{EditValue, EditValues};
use super::save::{RxSourceFile, SaveError, SaveHooks, SaveOutcome, save_database};

pub use libsodium_rs::crypto_secretbox::Key as StagingKey;

pub type EntrySnapshot = BTreeMap<String, Zeroizing<String>>;

#[derive(Default)]
pub enum Staging {
    #[default]
    None,
    Loaded(StagingKey),
    Renewing(StagingKey),
    Pending {
        current: StagingKey,
        next: StagingKey,
    },
    Renewed(StagingKey),
}

impl Staging {
    pub fn key(&self) -> Option<&StagingKey> {
        match self {
            Staging::None => None,
            Staging::Loaded(key)
            | Staging::Renewing(key)
            | Staging::Pending { current: key, .. }
            | Staging::Renewed(key) => Some(key),
        }
    }

    pub fn start_renewal(&mut self) -> bool {
        match take(self) {
            Staging::Loaded(key) => {
                *self = Staging::Renewing(key);
                true
            }
            other => {
                *self = other;
                false
            }
        }
    }

    pub fn hold(&mut self, next: StagingKey) {
        *self = match take(self) {
            Staging::Renewing(current) => Staging::Pending { current, next },
            other => other,
        };
    }

    pub fn commit(&mut self) {
        *self = match take(self) {
            Staging::Pending { next, .. } => Staging::Renewed(next),
            other => other,
        };
    }

    pub fn abandon(&mut self) {
        *self = match take(self) {
            Staging::Renewing(current) | Staging::Pending { current, .. } => {
                Staging::Renewed(current)
            }
            other => other,
        };
    }
}

#[derive(Serialize, Deserialize)]
pub struct StagedEdit {
    pub edit: RxEntryEdit,
    pub base_sha256: [u8; 32],
    pub base: EntrySnapshot,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

impl ChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Modified => "modified",
            ChangeKind::Deleted => "deleted",
        }
    }

    fn between(base: &str, value: &str) -> Self {
        match (base.is_empty(), value.is_empty()) {
            (true, false) => ChangeKind::Added,
            (false, true) => ChangeKind::Deleted,
            _ => ChangeKind::Modified,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct StagedChange {
    pub name: String,
    pub custom: bool,
    pub kind: ChangeKind,
}

impl StagedEdit {
    pub fn entry_changed(&self, sha256: &[u8; 32], current: Option<&EntrySnapshot>) -> bool {
        *sha256 != self.base_sha256 && current != Some(&self.base)
    }

    pub fn changes(&self) -> Vec<StagedChange> {
        let standard = [
            ("title", &self.edit.title),
            ("username", &self.edit.username),
            ("password", &self.edit.password),
            ("url", &self.edit.url),
            ("notes", &self.edit.notes),
        ];
        let fields = standard.into_iter().filter_map(|(key, value)| {
            let base = self.base.get(key).map_or("", |base| base.as_str());
            (base != value.as_str()).then(|| StagedChange {
                name: key.to_string(),
                custom: false,
                kind: ChangeKind::between(base, value),
            })
        });
        let custom_fields = self
            .edit
            .custom_fields
            .iter()
            .filter(|field| self.custom_field_changed(field))
            .map(|field| StagedChange {
                name: field.name.clone(),
                custom: true,
                kind: match field.original_name {
                    Some(_) => ChangeKind::Modified,
                    None => ChangeKind::Added,
                },
            });
        let removed = self.edit.removed_fields.iter().map(|name| StagedChange {
            name: name.clone(),
            custom: true,
            kind: ChangeKind::Deleted,
        });
        fields.chain(custom_fields).chain(removed).collect()
    }

    fn base_custom_field(&self, name: &str) -> Option<(&str, &str)> {
        let prefix = (0..)
            .map(|index| format!("customFields.{index}"))
            .map_while(|prefix| {
                let field_name = self.base.get(&format!("{prefix}.name"))?;
                Some((prefix, field_name))
            })
            .find(|(_, field_name)| field_name.as_str() == name)?
            .0;
        Some((
            self.base.get(&format!("{prefix}.value"))?.as_str(),
            self.base.get(&format!("{prefix}.protected"))?.as_str(),
        ))
    }

    fn custom_field_changed(&self, field: &RxFieldEdit) -> bool {
        let Some(original) = field.original_name.as_deref() else {
            return true;
        };
        original != field.name
            || self
                .base_custom_field(original)
                .is_none_or(|(value, protected)| {
                    value != field.value.as_str() || protected != field.protected.to_string()
                })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StageError {
    #[error("The unsaved edit could not be decrypted with this key")]
    Decrypt,
    #[error("Could not read the unsaved edit: {0}")]
    Open(DatabaseOpenError),
    #[error("Could not encrypt the unsaved edit: {0}")]
    Save(#[from] DatabaseSaveError),
    #[error("The unsaved edit is damaged: {0}")]
    Format(#[from] serde_json::Error),
    #[error("The unsaved edit file holds no edit")]
    Empty,
    #[error("The key for the unsaved edit is missing")]
    MissingKey,
    #[error("Could not write the unsaved edit: {0}")]
    Io(#[from] io::Error),
}

fn flatten(key: String, value: &EditValue, out: &mut EntrySnapshot) {
    match value {
        EditValue::Text(text) => {
            out.insert(key, Zeroizing::new(text.clone()));
        }
        EditValue::Flag(flag) => {
            out.insert(key, Zeroizing::new(flag.to_string()));
        }
        EditValue::List(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten(format!("{key}.{index}"), item, out);
            }
        }
        EditValue::Map(map) => {
            for (name, item) in map {
                flatten(format!("{key}.{name}"), item, out);
            }
        }
    }
}

pub fn snapshot(values: &EditValues) -> EntrySnapshot {
    let mut out = EntrySnapshot::new();
    for (key, value) in values {
        flatten(key.clone(), value, &mut out);
    }
    out
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = AtomicWriteFile::options().mode(0o600).open(path)?;
    file.write_all(bytes)?;
    file.commit()
}

fn read_private(path: &Path) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn write_secret(
    path: &Path,
    key: DatabaseKey,
    secret: &str,
    config: &DatabaseConfig,
) -> Result<(), StageError> {
    let mut db = Database::with_config(config.clone());
    db.config.version = DatabaseVersion::KDB4(1);
    db.root_mut().add_entry().edit(|entry| {
        entry.set_protected(fields::PASSWORD, secret);
    });

    let mut bytes = Zeroizing::new(Vec::new());
    db.save(&mut *bytes, key)?;
    Ok(write_private(path, &bytes)?)
}

fn read_secret(
    path: &Path,
    key: DatabaseKey,
) -> Result<Option<Zeroizing<String>>, StageError> {
    let Some(bytes) = read_private(path)? else {
        return Ok(None);
    };
    let db = Database::parse(&bytes, key).map_err(|err| match err {
        DatabaseOpenError::Key(DatabaseKeyError::IncorrectKey) => StageError::Decrypt,
        other => StageError::Open(other),
    })?;
    let entry = db.iter_all_entries().next().ok_or(StageError::Empty)?;
    let secret = entry.get(fields::PASSWORD).ok_or(StageError::Empty)?;
    Ok(Some(Zeroizing::new(secret.to_string())))
}

pub fn staging_key_path(staged: &Path) -> PathBuf {
    staged.with_extension("key")
}

pub fn open_staging_key(
    staged: &Path,
    key: DatabaseKey,
    config: &DatabaseConfig,
) -> Result<StagingKey, StageError> {
    let path = staging_key_path(staged);
    match read_secret(&path, key.clone()) {
        Ok(Some(encoded)) => {
            let bytes = Zeroizing::new(
                BASE64_STANDARD
                    .decode(encoded.as_bytes())
                    .map_err(|_| StageError::MissingKey)?,
            );
            return StagingKey::from_bytes(&bytes).map_err(|_| StageError::MissingKey);
        }
        Ok(None) if staged.exists() => return Err(StageError::MissingKey),
        Err(err) if staged.exists() => return Err(err),
        Ok(None) => {}
        Err(err) => warn!("Replacing the unreadable key for unsaved edits: {err}"),
    }
    write_staging_key(&path, key, config)
}

pub fn write_staging_key(
    path: &Path,
    key: DatabaseKey,
    config: &DatabaseConfig,
) -> Result<StagingKey, StageError> {
    let staging = StagingKey::generate();
    let encoded = Zeroizing::new(BASE64_STANDARD.encode(staging.as_bytes()));
    write_secret(path, key, &encoded, config)?;
    Ok(staging)
}

pub fn next_staging_key_path(staged: &Path) -> PathBuf {
    staged.with_extension("key.next")
}

pub fn replace_staging_key(staged: &Path) -> io::Result<()> {
    fs::rename(next_staging_key_path(staged), staging_key_path(staged))?;
    match staged.parent() {
        Some(dir) => fs::File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

pub fn write_staged(
    path: &Path,
    key: &StagingKey,
    staged: &StagedEdit,
) -> Result<(), StageError> {
    let json = Zeroizing::new(serde_json::to_vec(staged)?);
    let nonce = Nonce::generate();
    let mut bytes = Zeroizing::new(nonce.as_bytes().to_vec());
    bytes.extend(crypto_secretbox::seal(&json, &nonce, key));
    Ok(write_private(path, &bytes)?)
}

pub fn read_staged(path: &Path, key: &StagingKey) -> Result<Option<StagedEdit>, StageError> {
    let Some(bytes) = read_private(path)? else {
        return Ok(None);
    };
    if bytes.len() < NONCEBYTES {
        return Err(StageError::Decrypt);
    }
    let (nonce, sealed) = bytes.split_at(NONCEBYTES);
    let nonce = Nonce::try_from_slice(nonce).map_err(|_| StageError::Decrypt)?;
    let json = Zeroizing::new(
        crypto_secretbox::open(sealed, &nonce, key).map_err(|_| StageError::Decrypt)?,
    );
    Ok(Some(serde_json::from_slice(&json)?))
}

pub fn remove_staged(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub fn save_and_unstage(
    src: &RxSourceFile,
    key: DatabaseKey,
    backup: &Path,
    staged: &Path,
    edit: &RxEdit,
    hooks: &dyn SaveHooks,
) -> Result<SaveOutcome, SaveError> {
    let outcome = save_database(
        src,
        key,
        backup,
        |db| edit.apply(db).map_err(Into::into),
        |db| edit.verify(db).map_err(Into::into),
        hooks,
    )?;
    if let Err(err) = remove_staged(staged) {
        warn!("Saved, but could not remove the unsaved edit copy: {err}");
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "staged_tests.rs"]
mod tests;
