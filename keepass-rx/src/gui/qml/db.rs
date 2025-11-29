use actix::prelude::*;
use actor_macro::observing_model;
use anyhow::Result;
use qmeta_async::with_executor;
use qmetaobject::{QObjectPinned, prelude::*};
use std::{fs::read_to_string, fs::remove_file, path::Path};

use crate::{
    actor::{ActorConnected, ConnectedModelActor, ModelContext},
    app::{AppState, KeyFile, RxActors},
    gui::{
        RxDbType,
        actor::OpenDatabase,
        utils::{app_data_path, db_path_for_type},
    },
    rx::virtual_hierarchy::VirtualHierarchy,
};

#[derive(Message)]
#[rtype(result = "()")]
struct OpenCommand;

#[derive(Message)]
#[rtype(result = "()")]
struct UseKeyFileCommand {
    path: String,
    delete_key: bool,
}

#[observing_model]
#[derive(QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiDatabase {
    last_db_set: bool,
    key_file_set: bool,
    key_file_detected: bool,

    pub(super) base: qt_base_class!(trait QObject),
    pub(super) isLastDbSet: qt_property!(bool; READ get_last_db_set WRITE set_last_db_set NOTIFY isLastDbSetChanged),
    pub(super) isKeyFileSet: qt_property!(bool; READ get_key_file_set NOTIFY isKeyFileSetChanged),
    pub(super) isKeyFileDetected: qt_property!(bool; READ get_key_file_detected NOTIFY isKeyFileDetectedChanged),
    pub(super) databaseName: qt_property!(QString; NOTIFY databaseNameChanged),
    pub(super) databaseType: qt_property!(RxDbType; NOTIFY databaseTypeChanged),

    pub(super) databaseNameChanged: qt_signal!(),
    pub(super) databaseTypeChanged: qt_signal!(),
    pub(super) isLastDbSetChanged: qt_signal!(),
    pub(super) isKeyFileSetChanged: qt_signal!(),
    pub(super) isKeyFileDetectedChanged: qt_signal!(),

    pub(super) useKeyFile: qt_method!(fn(&mut self, key_file_path: QString)),
    pub(super) clearKeyFile: qt_method!(fn(&mut self)),
    pub(super) open: qt_method!(fn(&self)),
    pub(super) updateLastDbSet: qt_method!(fn(&mut self)),
    pub(super) detectKeyFile: qt_method!(fn(&self)),
    pub(super) databaseTypeTranslated: qt_property!(QString; READ translate_db_type NOTIFY databaseTypeChanged),
}

impl ActorConnected<UseKeyFileCommand> for RxUiDatabase {
    type Context = ModelContext<Self>;

    fn app_state(&self) -> &crate::app::AppState {
        self._app.as_ref().expect("No app state available")
    }

    fn handle(&mut self, _: Self::Context, message: UseKeyFileCommand)
    where
        Self: Sized + QObject,
    {
        let key_file_path = message.path;
        self.set_key_file(&key_file_path);

        // Remove the file, if it exists. Only do this when importing
        // from ContentHub; we don't want to keep the key file on disk
        // in data dir.
        if message.delete_key {
            if let Err(err) = std::fs::remove_file(&key_file_path) {
                println!("Could not remove imported key file: {}", err);
            }
        }
    }
}

impl ActorConnected<OpenCommand> for RxUiDatabase {
    type Context = ModelContext<Self>;

    fn app_state(&self) -> &crate::app::AppState {
        self._app.as_ref().expect("No app state available")
    }

    fn handle(&mut self, _: Self::Context, _: OpenCommand)
    where
        Self: Sized + QObject,
    {
        let app_actor = RxActors::app_actor().expect("No actor");

        actix::spawn(app_actor.send(OpenDatabase {
            db_name: self.databaseName.to_string(),
            db_type: self.databaseType,
        }));
    }
}

fn write_last_db(db_name: String, db_type: RxDbType) -> Result<()> {
    let last_db_file = app_data_path().join("last-db");
    let last_db_type = app_data_path().join("last-db-type");
    std::fs::write(last_db_file, db_name)?;
    std::fs::write(last_db_type, db_type.to_string())?;

    Ok(())
}

fn clear_last_db() -> Result<()> {
    let last_db_file = app_data_path().join("last-db");
    let last_db_type = app_data_path().join("last-db-type");

    if last_db_file.exists() {
        remove_file(last_db_file)?;
    }

    if last_db_type.exists() {
        remove_file(last_db_type)?;
    }

    Ok(())
}

fn load_last_db() -> Result<(Option<String>, Option<String>)> {
    let last_db_file = app_data_path().join("last-db");
    let last_db_type = app_data_path().join("last-db-type");

    // Load the last DB and its recorded type from file. Supports a
    // few scenarios: both defined, only the db file defined (from old
    // app versions), or neither defined.
    let files = match (last_db_file, last_db_type) {
        (db, db_type) if db.exists() && db_type.exists() => {
            // both recorded
            (Some(read_to_string(db)?), Some(read_to_string(db_type)?))
        }
        (db, db_type) if db.exists() && !db_type.exists() => {
            // only db file recorded (from old app versions)
            println!("No last DB type. Assuming last DB is Imported");
            (Some(read_to_string(db)?), None)
        }
        _ => {
            // nothing recorded
            (None, None)
        }
    };

    if let (Some(ref db_file), ref maybe_db_type) = files {
        // File must actually exist to be valid.
        let db_type = maybe_db_type
            .as_ref()
            .map(|db_type| RxDbType::try_from(db_type))
            .transpose()?
            .unwrap_or_default();

        let data_dir = db_path_for_type(db_type);

        match data_dir.join(db_file) {
            path if path.exists() => Ok(files),
            _ => Ok((None, None)),
        }
    } else {
        Ok(files)
    }
}

impl Default for RxUiDatabase {
    fn default() -> Self {
        let (last_db, last_db_type) = load_last_db().unwrap();
        let have_last_db = last_db.is_some();

        Self {
            clearKeyFile: Default::default(),
            isKeyFileSet: Default::default(),
            isKeyFileDetected: Default::default(),
            isKeyFileDetectedChanged: Default::default(),
            key_file_detected: Default::default(),
            _app: Default::default(),
            _connected_model_registration: Default::default(),
            _ready: Default::default(),
            _readyChanged: Default::default(),
            base: Default::default(),
            databaseName: last_db.map(QString::from).unwrap_or_default(),
            databaseType: last_db_type
                .and_then(|db_type| RxDbType::try_from(db_type).ok())
                .unwrap_or_default(),
            databaseNameChanged: Default::default(),
            databaseTypeChanged: Default::default(),
            last_db_set: have_last_db,
            key_file_set: Default::default(),
            isLastDbSet: Default::default(),
            isLastDbSetChanged: Default::default(),
            isKeyFileSetChanged: Default::default(),
            useKeyFile: Default::default(),
            detectKeyFile: Default::default(),
            open: Default::default(),
            updateLastDbSet: Default::default(),
            databaseTypeTranslated: Default::default(),
        }
    }
}

#[allow(non_snake_case)]
impl RxUiDatabase {
    fn init_from_state(&mut self, _: &AppState) {
        self.detectKeyFile();
    }

    fn init_from_view(&mut self, _: &dyn VirtualHierarchy) {}

    fn app_state_cell(&self) -> QObjectPinned<'_, crate::app::AppState> {
        self._app.as_pinned().expect("No app state")
    }

    fn connected_actor(&self) -> Option<Addr<ConnectedModelActor<Self>>> {
        self._connected_model_registration
            .as_ref()
            .map(|reg| reg.actor.clone())
    }

    fn set_key_file(&mut self, key_file_path: impl AsRef<Path>) {
        let read_key = move || -> Result<Vec<u8>> {
            let source = key_file_path.as_ref();
            let key_bytes = std::fs::read(source)?;
            Ok(key_bytes)
        };

        let is_key_currently_set = self.key_file_set;

        match read_key() {
            Ok(key_bytes) => {
                let app_state = self.app_state_cell();
                let mut app_state = app_state.borrow_mut();
                app_state.set_db_key(Some(KeyFile::Unencrypted(key_bytes)));
                drop(app_state);
                self.key_file_set = true;
            }
            Err(err) => {
                println!("Error setting key file: {}", err);
                self.key_file_set = false;
            }
        }

        if self.key_file_set != is_key_currently_set {
            self.isKeyFileSetChanged();
        }
    }

    fn get_last_db_set(&self) -> bool {
        self.last_db_set
    }

    fn get_key_file_set(&self) -> bool {
        self.key_file_set
    }

    fn get_key_file_detected(&self) -> bool {
        self.key_file_detected
    }

    fn updateLastDbSet(&mut self) {
        let is_set = !self.databaseName.is_null() && !self.databaseName.is_empty();
        let change = is_set != self.last_db_set;

        if is_set != self.last_db_set {
            self.last_db_set = is_set;
        }

        self.write_last_db_files();

        if change {
            self.isLastDbSetChanged();
        }
    }

    fn set_last_db_set(&mut self, value: bool) {
        if value != self.last_db_set {
            self.last_db_set = value;
            self.write_last_db_files();
            self.isLastDbSetChanged();
        }
    }

    fn write_last_db_files(&self) {
        // Write to file if we have it.
        if self.last_db_set {
            let res = write_last_db(self.databaseName.to_string(), self.databaseType);

            if let Err(err) = res {
                println!("{}", err);
            }
        } else {
            // Nuke the settings files if we are explicitly un-setting
            // last db.
            if let Err(err) = clear_last_db() {
                println!("{}", err);
            }
        }
    }

    #[with_executor]
    pub fn detectKeyFile(&mut self) {
        // Only do auto-detection for synced DBs.
        if self.databaseType != RxDbType::Synced {
            self.clearKeyFile();
            return;
        }

        let data_dir = db_path_for_type(self.databaseType);
        let binding = self.databaseName.to_string();
        let db_raw_name = binding.split(".kdbx").next();

        let key_file_name = db_raw_name.and_then(|name| {
            if name.len() > 0 {
                let mut name = name.to_string();
                name.push_str(".key");
                Some(name)
            } else {
                None
            }
        });

        let maybe_key_path = key_file_name.map(|filename| data_dir.join(filename));

        if let Some(key_file) = maybe_key_path.as_ref()
            && key_file.exists()
        {
            println!(
                "Found a companion key file for: {}",
                self.databaseName.to_string()
            );

            let is_key_currently_detected = self.key_file_detected;
            self.key_file_detected = true;
            if is_key_currently_detected != self.key_file_detected {
                self.isKeyFileDetectedChanged();
            }

            if let Some(actor) = self.connected_actor() {
                actix::spawn(actor.send(UseKeyFileCommand {
                    path: key_file.to_string_lossy().to_string(),
                    delete_key: false,
                }));
            } else {
                self.set_key_file(&key_file);
            }
        } else {
            self.clearKeyFile();
        }
    }

    /// This method must be fully synchronous to prevent the content
    /// hub from deleting the file on finalize() before we copy it.
    #[with_executor]
    pub fn useKeyFile(&mut self, key_file_path: QString) {
        println!("Attempting to use key file: {:?}", key_file_path);
        let key_file_path = key_file_path.to_string();
        self.set_key_file(&key_file_path);

        // Remove the file, if it exists. Only do this when importing
        // from ContentHub; we don't want to keep the key file on disk
        // in data dir.
        if let Err(err) = std::fs::remove_file(&key_file_path) {
            println!("Could not remove imported key file: {}", err);
        }
    }

    #[with_executor]
    pub fn clearKeyFile(&mut self) {
        let is_key_file_currently_set = self.key_file_set;
        let is_key_file_currently_detected = self.key_file_detected;

        let app_state = self.app_state_cell();
        let mut app_state = app_state.borrow_mut();
        app_state.set_db_key(None);
        app_state.set_master_key(None);
        drop(app_state);

        self.key_file_detected = false;
        self.key_file_set = false;

        if is_key_file_currently_detected != self.key_file_detected {
            self.isKeyFileDetectedChanged();
        }

        if is_key_file_currently_set != self.key_file_set {
            self.isKeyFileSetChanged();
        }
    }

    #[with_executor]
    fn open(&self) {
        // Communicate with global object here. Stopgap.
        if let Some(actor) = self.connected_actor() {
            actix::spawn(actor.send(OpenCommand));
        } else {
            println!("No actor connection active?");
        }
    }

    pub fn translate_db_type(&self) -> QString {
        self.databaseType.translate().into()
    }
}
