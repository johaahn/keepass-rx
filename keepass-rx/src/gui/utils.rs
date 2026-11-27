use anyhow::Result;
use log::{debug, error, info, warn};
use std::path::PathBuf;

use dirs::data_dir;

use std::str::FromStr;
use uuid::Uuid;

use secstr::SecUtf8;
use zeroize::Zeroize;

use super::RxDbType;
use super::actor::{SaveCredential, SaveEntry};
use crate::rx::{RxEntryEdit, edit_values_from_map, validate_edit_values};
use qmetaobject::{QString, QVariant, QVariantList, QVariantMap};

#[cfg(not(feature = "sailfish"))]
const APP_ID: &'static str = "keepassrx.projectmoon";
// Sailjail confines the app to ~/.local/share/<desktop-name>, so on Sailfish the
// data directory must match the harbour app name.
#[cfg(feature = "sailfish")]
const APP_ID: &'static str = "harbour-keepassrx";

/// Old messed up data path, that had the app ID twice.
pub fn app_data_path() -> PathBuf {
    let data_dir = data_dir().expect("no data dir?");
    PathBuf::from(data_dir).join(APP_ID)
}

pub fn imported_databases_path() -> PathBuf {
    PathBuf::from(app_data_path()).join("imported")
}

pub fn synced_databases_path() -> PathBuf {
    PathBuf::from(app_data_path()).join("synced")
}

pub fn exported_attachments_path() -> PathBuf {
    PathBuf::from(app_data_path()).join("exports")
}

pub fn db_path_for_type(db_type: RxDbType) -> PathBuf {
    match db_type {
        RxDbType::Imported => imported_databases_path(),
        RxDbType::Synced => synced_databases_path(),
    }
}

/// Perform random migrations and file moves from old versions of the
/// app. This is not a proper migration facility, but rather a quick
/// hack job to update directory structures to what they should be.
pub fn move_old_dirs_and_files() -> Result<()> {
    // Original version of the app stored only one database at
    // db.kdbx. This moves it into the new multiple DB directory
    // structure.
    let old_db_path = app_data_path().join("db.kdbx");

    if old_db_path.exists() {
        let dest = imported_databases_path().join("db.kdbx");
        match std::fs::copy(&old_db_path, dest) {
            Ok(_) => {
                info!("Copied old db.kdbx to imported directory");
                if let Err(err) = std::fs::remove_file(old_db_path) {
                    warn!("Failed to remove old db.kdbx: {}", err);
                }
            }
            Err(err) => error!("Failed to copy old db.kdbx: {}", err),
        }
    }

    // Older versions of the app had a messed up data directory that
    // had the app ID twice: keepassrx.projectmoon/keepassrx.projectmoon.
    // This was caused by the synced and imported db path functions
    // appending APP_ID after calling app_data_dir().
    let incorrect_data_dir = app_data_path().join(APP_ID);
    if incorrect_data_dir.exists() {
        // Move all files and folders in this directory up one.
        let parent = incorrect_data_dir
            .parent()
            .expect("Old data directory has no parent?");

        info!(
            "Moving data from incorrect data directory: {:?}",
            std::fs::canonicalize(&incorrect_data_dir)?
        );

        for entry in std::fs::read_dir(&incorrect_data_dir)? {
            debug!("Processing entry: {:?}", entry);
            let entry = entry?;
            let src = std::fs::canonicalize(entry.path())?;
            let dest = parent.join(&entry.file_name());
            let abs_dst = parent.join(&src).canonicalize();
            let abs_dst = abs_dst.as_ref().unwrap_or(&dest);

            info!("Moving '{}' to: '{}'", src.display(), abs_dst.display());

            // If destination already exists, nuke it.
            match dest {
                ref dir if dir.exists() && dir.is_dir() => {
                    warn!("Removing existing directory: {}", dir.display());
                    std::fs::remove_dir_all(dir)?;
                }
                ref file if file.exists() => {
                    warn!("Removing existing file: {}", file.display());
                    std::fs::remove_file(&file)?;
                }
                _ => (),
            }

            std::fs::rename(src, dest)?;
        }

        // Now finally remove the bad data dir.
        std::fs::remove_dir_all(&incorrect_data_dir)?;
    }

    Ok(())
}

pub fn save_entry_message(
    entry_uuid: &QString,
    values: QVariantMap,
    credential: &QString,
) -> Result<SaveEntry, String> {
    let uuid = Uuid::from_str(&entry_uuid.to_string()).map_err(|err| err.to_string())?;
    let edit = RxEntryEdit::from_values(uuid, &edit_values_from_map(values))
        .map_err(|err| err.to_string())?;
    Ok(SaveEntry {
        edit,
        credential: SaveCredential::from_input(&credential.to_string()),
    })
}

pub fn entry_edit_problems(values: QVariantMap) -> QVariantList {
    validate_edit_values(&edit_values_from_map(values))
        .into_iter()
        .filter_map(|problem| {
            let index = i32::try_from(problem.index?).ok()?;
            let mut map = QVariantMap::default();
            map.insert("index".into(), QVariant::from(index));
            map.insert("part".into(), QVariant::from(QString::from("name")));
            map.insert(
                "message".into(),
                QVariant::from(QString::from(problem.message)),
            );
            Some(QVariant::from(map))
        })
        .collect()
}

pub fn qstring_to_secure_utf8(value: &QString) -> SecUtf8 {
    let utf16 = value.to_slice();
    let mut utf8 = Vec::with_capacity(utf16.len());

    for ch in char::decode_utf16(utf16.iter().copied()) {
        let ch = ch.unwrap_or(char::REPLACEMENT_CHARACTER);
        let mut buf = [0u8; 4];
        utf8.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
    }

    let secure = std::str::from_utf8(&utf8)
        .ok()
        .and_then(|s| SecUtf8::from_str(s).ok())
        .unwrap_or_else(|| SecUtf8::from(""));

    utf8.zeroize();
    secure
}
