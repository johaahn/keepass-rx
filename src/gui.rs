use actix::prelude::*;
use anyhow::{Result, anyhow};
use dirs::data_dir;
use keepass::{Database, DatabaseKey};
use qmeta_async::with_executor;
use qmetaobject::*;
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::rx::RxDatabase;

const APP_ID: &'static str = "keepassrx.projectmoon";

fn app_data_path() -> PathBuf {
    let data_dir = data_dir().expect("no data dir?");
    PathBuf::from(data_dir).join(APP_ID)
}

#[derive(Clone, Default)]
pub struct KeepassRxActor {
    gui: Arc<QObjectBox<KeepassRX>>,
    curr_db: Rc<Option<RxDatabase>>,
}

impl KeepassRxActor {
    pub fn new(gui: &Arc<QObjectBox<KeepassRX>>) -> Self {
        Self {
            gui: gui.clone(),
            curr_db: Default::default(),
        }
    }

    pub fn set_file(&self, path: &str, _is_db: bool) -> Result<()> {
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

    fn db(&self) -> Result<&RxDatabase> {
        // rc as_ref -> option as_ref
        Ok(self
            .curr_db
            .as_ref()
            .as_ref()
            .ok_or(anyhow!("Database not open"))?)
    }
}

impl Actor for KeepassRxActor {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.gui.pinned().borrow_mut().actor = Some(ctx.address());
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct SetFile {
    path: String,
    picking_db: bool,
}

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
struct OpenDatabase {
    path: String,
    password: String,
    key_path: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct GetEntries {
    search_term: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct GetGroups;

#[derive(Message)]
#[rtype(result = "()")]
struct GetTotp {
    entry_uuid: String,
}

impl Handler<SetFile> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, msg: SetFile, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        match self.set_file(&msg.path, msg.picking_db) {
            Ok(_) => gui.fileSet(msg.path),
            Err(err) => gui.databaseOpenFailed(format!("{}", err)),
        }
    }
}

impl Handler<OpenDatabase> for KeepassRxActor {
    type Result = AtomicResponse<Self, anyhow::Result<()>>;
    fn handle(&mut self, msg: OpenDatabase, _: &mut Self::Context) -> Self::Result {
        AtomicResponse::new(Box::pin(
            async move {
                let mut db_file = File::open(msg.path)?;
                let key_file = msg.key_path.map(|p| File::open(p));

                let db_key = DatabaseKey::new().with_password(&msg.password);
                let db_key = match key_file {
                    // the double ? coerces File::open result and with_keyfile result.
                    Some(file) => db_key.with_keyfile(&mut file?)?,
                    None => db_key,
                };

                let open_result =
                    tokio::task::spawn_blocking(move || -> Result<RxDatabase> {
                        let db = Database::open(&mut db_file, db_key)?;
                        let mut rx_db = RxDatabase::new(db);
                        rx_db.load_data();
                        Ok(rx_db)
                    })
                    .await??;

                Ok(open_result)
            }
            .into_actor(self)
            .map(|result: Result<RxDatabase>, this, _| {
                let binding = this.gui.clone();
                let binding = binding.pinned();
                let gui = binding.borrow();

                match result {
                    Ok(rx_db) => {
                        this.curr_db = Rc::new(Some(rx_db));
                        gui.databaseOpened();
                    }
                    Err(err) => gui.databaseOpenFailed(format!("{}", err)),
                }

                Ok(())
            }),
        ))
    }
}

impl Handler<GetGroups> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, _: GetGroups, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db = match self.db() {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let groups: QStringList = db
            .groups()
            .into_iter()
            .map(|group| group.name.clone())
            .collect();

        gui.groupsReceived(groups);
    }
}

impl Handler<GetEntries> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetEntries, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db = match self.db() {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let entries = db.get_entries(msg.search_term.as_deref());
        let map: HashMap<String, QVariantList> = entries
            .into_iter()
            .map(|(group_name, entries)| {
                let qvariants = entries.into_iter().map(|ent| Into::<QVariant>::into(ent));
                let qvariant_list = QVariantList::from_iter(qvariants);
                (group_name, qvariant_list)
            })
            .collect();

        gui.entriesReceived(QVariantMap::from(map));
    }
}

impl Handler<GetTotp> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetTotp, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db = match self.db() {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let totp = db.get_totp(&msg.entry_uuid);

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

        gui.totpReceived(QVariantMap::from(map));
    }
}

#[derive(QObject, Default)]
#[allow(non_snake_case)]
pub struct KeepassRX {
    base: qt_base_class!(trait QObject),
    actor: Option<Addr<KeepassRxActor>>,

    setFile: qt_method!(fn(&self, path: String, is_db: bool)),
    openDatabase: qt_method!(fn(&mut self, path: String, password: String, key_path: QString)),
    getGroups: qt_method!(fn(&self)),
    getEntries: qt_method!(fn(&self, search_term: QString)),
    getTotp: qt_method!(fn(&self, entry_uuid: QString)),

    // signals
    fileSet: qt_signal!(path: String),
    databaseOpened: qt_signal!(),
    databaseOpenFailed: qt_signal!(message: String),
    groupsReceived: qt_signal!(groups: QStringList),
    entriesReceived: qt_signal!(entries: QVariantMap),
    errorReceived: qt_signal!(error: String),
    totpReceived: qt_signal!(totp: QVariantMap),
}

#[allow(non_snake_case)]
impl KeepassRX {
    #[with_executor]
    pub fn setFile(&self, path: String, is_db: bool) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(SetFile {
            path,
            picking_db: is_db,
        }));
    }

    #[with_executor]
    pub fn openDatabase(&mut self, path: String, password: String, key_path: QString) {
        let key_path = match key_path {
            kp if !kp.is_null() && !kp.is_empty() => Some(kp.to_string()),
            _ => None,
        };

        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(OpenDatabase {
            path,
            password,
            key_path,
        }));
    }

    #[with_executor]
    pub fn getGroups(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetGroups));
    }

    #[with_executor]
    pub fn getEntries(&self, search_term: QString) {
        let search_term = match search_term {
            term if !term.is_null() => Some(term.to_string()),
            _ => None,
        };

        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetEntries { search_term }));
    }

    #[with_executor]
    pub fn getTotp(&self, entry_uuid: QString) {
        let entry_uuid = entry_uuid.to_string();
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetTotp { entry_uuid }));
    }
}
