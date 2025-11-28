use actix::prelude::*;
use actor_macro::observing_model;
use anyhow::{Result, anyhow};
use qmeta_async::with_executor;
use qmetaobject::prelude::*;
use std::{fs::read_to_string, fs::remove_file, path::Path};

use crate::{
    actor::{ActorConnected, ConnectedModelActor, ModelContext},
    app::RxActors,
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

#[observing_model]
#[derive(QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiDatabase {
    last_db_set: bool,

    pub(super) base: qt_base_class!(trait QObject),
    pub(super) isLastDbSet: qt_property!(bool; READ get_last_db_set WRITE set_last_db_set NOTIFY isLastDbSetChanged),
    pub(super) databaseName: qt_property!(QString; NOTIFY databaseNameChanged),
    pub(super) databaseType: qt_property!(RxDbType; NOTIFY databaseTypeChanged),

    pub(super) databaseNameChanged: qt_signal!(),
    pub(super) databaseTypeChanged: qt_signal!(),
    pub(super) isLastDbSetChanged: qt_signal!(),

    pub(super) open: qt_method!(fn(&self)),
    pub(super) updateLastDbSet: qt_method!(fn(&mut self)),
    pub(super) databaseTypeTranslated: qt_property!(QString; READ translate_db_type NOTIFY databaseTypeChanged),
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
            key_path: None,
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
            isLastDbSet: Default::default(),
            isLastDbSetChanged: Default::default(),
            open: Default::default(),
            updateLastDbSet: Default::default(),
            databaseTypeTranslated: Default::default(),
        }
    }
}

#[allow(non_snake_case)]
impl RxUiDatabase {
    fn init_from_view(&mut self, _: &dyn VirtualHierarchy) {}

    fn connected_actor(&self) -> Option<Addr<ConnectedModelActor<Self>>> {
        self._connected_model_registration
            .as_ref()
            .map(|reg| reg.actor.clone())
    }

    fn get_last_db_set(&self) -> bool {
        self.last_db_set
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
