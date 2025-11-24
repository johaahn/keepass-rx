use std::path::PathBuf;

use dirs::data_dir;

const APP_ID: &'static str = "keepassrx.projectmoon";

pub fn app_data_path() -> PathBuf {
    let data_dir = data_dir().expect("no data dir?");
    PathBuf::from(data_dir).join(APP_ID)
}

pub fn imported_databases_path() -> PathBuf {
    PathBuf::from(app_data_path()).join(APP_ID).join("imported")
}

pub fn synced_databases_path() -> PathBuf {
    PathBuf::from(app_data_path()).join(APP_ID).join("synced")
}

pub fn move_old_db() {
    let db_path = app_data_path().join("db.kdbx");

    if db_path.exists() {
        let dest = imported_databases_path().join("db.kdbx");
        match std::fs::copy(&db_path, dest) {
            Ok(_) => {
                println!("Copied old db.kdbx to imported directory");
                if let Err(err) = std::fs::remove_file(db_path) {
                    println!("Failed to remove old db.kdbx: {}", err);
                }
            }
            Err(err) => println!("Failed to copy old db.kdbx: {}", err),
        }
    }
}
