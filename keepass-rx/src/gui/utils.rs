use anyhow::Result;
use std::path::PathBuf;

use dirs::data_dir;

use super::RxDbType;

const APP_ID: &'static str = "keepassrx.projectmoon";

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
                println!("Copied old db.kdbx to imported directory");
                if let Err(err) = std::fs::remove_file(old_db_path) {
                    println!("Failed to remove old db.kdbx: {}", err);
                }
            }
            Err(err) => println!("Failed to copy old db.kdbx: {}", err),
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

        println!(
            "Moving data from incorrect data directory: {:?}",
            std::fs::canonicalize(&incorrect_data_dir)?
        );

        for entry in std::fs::read_dir(&incorrect_data_dir)? {
            println!("Processing entry: {:?}", entry);
            let entry = entry?;
            let src = std::fs::canonicalize(entry.path())?;
            let dest = parent.join(&entry.file_name());
            let abs_dst = parent
                .join(&src)
                .canonicalize()
                .unwrap_or_else(|_| dest.clone());

            println!("Moving '{}' to: '{}'", src.display(), abs_dst.display());

            // If destination already exists, nuke it.
            match dest {
                ref dir if dir.exists() && dir.is_dir() => {
                    println!("Removing existing directory: {}", dir.display());
                    std::fs::remove_dir_all(dir)?;
                }
                ref file if file.exists() => {
                    println!("Removing existing file: {}", file.display());
                    std::fs::remove_file(&file)?;
                }
                _ => (),
            }

            std::fs::rename(src, dest)?;
        }

        // Now finally remove the bad data dir.
        std::fs::remove_file(&incorrect_data_dir)?;
    }

    Ok(())
}
