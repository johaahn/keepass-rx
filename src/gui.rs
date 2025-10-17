use anyhow::{anyhow, Result};
use dirs::data_dir;
use keepass::{Database, DatabaseKey};
use qmetaobject::*;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::rx::RxDatabase;

const APP_ID: &'static str = "keepassrx.projectmoon";

fn app_data_path() -> PathBuf {
    let data_dir = data_dir().expect("no data dir?");
    PathBuf::from(data_dir).join(APP_ID)
}

#[derive(QObject, Default)]
#[allow(non_snake_case)]
pub struct KeepassRX {
    base: qt_base_class!(trait QObject),

    curr_db: Rc<Option<RxDatabase>>,

    setFile: qt_method!(fn(&self, path: String, is_db: bool)),
    openDatabase: qt_method!(fn(&mut self, path: String, password: String, key_path: QString)),
    getGroups: qt_method!(fn(&self) -> QStringList),
    getEntries: qt_method!(fn(&self, search_term: QString) -> QVariantMap),
    getTotp: qt_method!(fn(&self, entry_uuid: QString) -> QVariantMap),
    databaseOpened: qt_signal!(),
    databaseOpenFailure: qt_signal!(message: String),
}

#[allow(non_snake_case)]
impl KeepassRX {
    pub fn set_file(&self, path: String, is_db: bool) -> Result<()> {
        let source = Path::new(&path);
        let dest_dir = app_data_path();
        let dest = dest_dir.join("db.kdbx");

        if source == dest {
            return Err(anyhow!("Trying to copy source to the same destination"));
        }

        println!("Making directory: {}", dest_dir.display());
        create_dir_all(&dest_dir)?;

        println!(
            "Copying database from {} to {}",
            source.display(),
            dest.display()
        );

        let bytes_copied = std::fs::copy(&source, &dest)?;
        println!("Copied {} bytes", bytes_copied);
        Ok(())
    }

    pub fn setFile(&self, path: String, is_db: bool) {
        if let Err(err) = self.set_file(path, is_db) {
            println!("Error copying data: {:?}", err);
        }
    }

    pub fn open_database(
        &mut self,
        path: String,
        password: String,
        key_path: Option<String>,
    ) -> Result<()> {
        let mut db_file = File::open(path)?;
        let key_file = key_path.map(|p| File::open(p));

        let db_key = DatabaseKey::new().with_password(&password);
        let db_key = match key_file {
            // the double ? coerces File::open result and with_keyfile result.
            Some(file) => db_key.with_keyfile(&mut file?)?,
            None => db_key,
        };

        let db = Database::open(&mut db_file, db_key)?;
        let mut rx_db = RxDatabase::new(db);
        rx_db.load_data();

        self.curr_db = Rc::new(Some(rx_db));

        Ok(())
    }

    pub fn openDatabase(&mut self, path: String, password: String, key_path: QString) {
        let key_path = match key_path {
            kp if !kp.is_null() && !kp.is_empty() => Some(kp.to_string()),
            _ => None,
        };

        match self.open_database(path, password, key_path) {
            Ok(_) => self.databaseOpened(),
            Err(err) => self.databaseOpenFailure(err.to_string()),
        };
    }

    fn db(&self) -> &RxDatabase {
        // rc as_ref -> option as_ref
        self.curr_db.as_ref().as_ref().expect("Database not open")
    }

    pub fn getGroups(&self) -> QStringList {
        self.db()
            .groups()
            .into_iter()
            .map(|group| group.name.clone())
            .collect()
    }

    pub fn getEntries(&self, search_term: QString) -> QVariantMap {
        let search_term = match search_term {
            term if !term.is_null() => Some(term.to_string()),
            _ => None,
        };

        let entries = self.db().get_entries(search_term.as_deref());

        let map: HashMap<String, QVariantList> = entries
            .into_iter()
            .map(|(group_name, entries)| {
                let qvariants = entries.into_iter().map(|ent| Into::<QVariant>::into(ent));
                let qvariant_list = QVariantList::from_iter(qvariants);
                (group_name, qvariant_list)
            })
            .collect();

        QVariantMap::from(map)
    }

    pub fn getTotp(&self, entry_uuid: QString) -> QVariantMap {
        let totp = self.db().get_totp(&entry_uuid.to_string());

        let mut map: HashMap<String, QVariant> = HashMap::new();
        match totp {
            Ok(otp) => {
                let digits = QString::from(otp.code);
                let valid_for = QString::from(otp.valid_for);

                map.insert("digits".to_string(), digits.into());
                map.insert("validFor".to_string(), valid_for.into());
            }
            Err(err) => {
                map.insert(
                    "error".to_string(),
                    QString::from(format!("{}", err)).into(),
                );
            }
        }

        QVariantMap::from(map)
    }
}
