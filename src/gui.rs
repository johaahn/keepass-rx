use actix::prelude::*;
use anyhow::{Result, anyhow};
use dirs::data_dir;
use keepass::{Database, DatabaseKey};
use qmeta_async::with_executor;
use qmetaobject::*;
use secstr::SecUtf8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{File, create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::{JoinHandle, spawn_blocking};
use zeroize::{Zeroize, Zeroizing};

use crate::rx::{EncryptedPassword, RxDatabase, ZeroableDatabase};

const APP_ID: &'static str = "keepassrx.projectmoon";

fn app_data_path() -> PathBuf {
    let data_dir = data_dir().expect("no data dir?");
    PathBuf::from(data_dir).join(APP_ID)
}

#[derive(Default)]
pub struct KeepassRxActor {
    gui: Arc<QObjectBox<KeepassRx>>,
    curr_db: RefCell<Option<RxDatabase>>,
    curr_master_pw: Arc<RefCell<Option<EncryptedPassword>>>,
    stored_master_password: Arc<RefCell<Option<SecUtf8>>>,

    // any in-progress operation on another thread pool that might
    // need to be aborted.
    current_operation: Option<JoinHandle<Result<()>>>,
}

impl KeepassRxActor {
    pub fn new(gui: &Arc<QObjectBox<KeepassRx>>) -> Self {
        Self {
            gui: gui.clone(),
            ..Default::default()
        }
    }

    pub fn abort_ongoing_operations(&self) {
        // Abort any in-progress encryption operation.
        if let Some(ref in_progress) = self.current_operation {
            if !in_progress.is_finished() {
                in_progress.abort();
                println!("Aborting ongoing encryption/decryption operation.");
            }
        }
    }
}

impl Actor for KeepassRxActor {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.gui.pinned().borrow_mut().actor = Some(ctx.address());
    }
}

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
struct OpenDatabase {
    path: String,
    key_path: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct CloseDatabase;

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

#[derive(Message)]
#[rtype(result = "()")]
struct StoreMasterPassword {
    master_password: SecUtf8,
}

#[derive(Message)]
#[rtype(result = "()")]
struct EncryptMasterPassword;

#[derive(Message)]
#[rtype(result = "()")]
struct DecryptMasterPassword {
    short_password: SecUtf8,
}

#[derive(Message)]
#[rtype(result = "()")]
struct InvalidateMasterPassword;

#[derive(Message)]
#[rtype(result = "()")]
struct CheckLockingStatus;

impl Handler<OpenDatabase> for KeepassRxActor {
    type Result = AtomicResponse<Self, anyhow::Result<()>>;
    fn handle(&mut self, msg: OpenDatabase, _: &mut Self::Context) -> Self::Result {
        // Opening the database is synchronous I/O, which means it
        // must be done on a separate thread.

        // Clone here so we can encrypt later.
        let stored_pw = self.stored_master_password.clone();

        AtomicResponse::new(Box::pin(
            async move {
                let pw_binding = stored_pw.borrow();
                let pw_binding = pw_binding.as_ref();
                let pw_binding = pw_binding
                    .as_deref()
                    .ok_or(anyhow!("No master password stored"))?;

                let mut db_file = File::open(msg.path)?;
                let key_file = msg.key_path.map(|p| File::open(p));

                let db_key = DatabaseKey::new().with_password(pw_binding.unsecure());
                let db_key = match key_file {
                    // the double ? coerces File::open result and with_keyfile result.
                    Some(file) => db_key.with_keyfile(&mut file?)?,
                    None => db_key,
                };

                let open_result = spawn_blocking(move || -> Result<RxDatabase> {
                    let db = Database::open(&mut db_file, db_key)?;
                    let wrapped_db = Zeroizing::new(ZeroableDatabase(db));
                    let rx_db = RxDatabase::new(wrapped_db);
                    Ok(rx_db)
                })
                .await??;

                Ok(open_result)
            }
            .into_actor(self)
            .map(|result: Result<RxDatabase>, this, _| {
                let binding = this.gui.clone();
                let binding = binding.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(rx_db) => {
                        this.curr_db = RefCell::new(Some(rx_db));
                        gui.databaseOpen = true;
                        gui.databaseOpened();
                    }
                    Err(err) => gui.databaseOpenFailed(format!("{}", err)),
                }

                Ok(())
            }),
        ))
    }
}

impl Handler<CloseDatabase> for KeepassRxActor {
    type Result = AtomicResponse<Self, ()>;

    fn handle(&mut self, _: CloseDatabase, _: &mut Self::Context) -> Self::Result {
        // Remove from cell
        let db = self.curr_db.take();
        AtomicResponse::new(Box::pin(
            async move {
                // Remove from option.
                let mut db = db.ok_or(anyhow!("Database not open"))?;
                db.close();
                Ok(())
            }
            .into_actor(self)
            .map(|result: Result<()>, this, _| {
                let binding = this.gui.clone();
                let binding = binding.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(_) => {
                        gui.databaseOpen = false;
                        gui.databaseClosed();
                    }
                    Err(err) => gui.errorReceived(format!("{}", err)),
                };
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

        let db_binding = self.curr_db.borrow();
        let db = match db_binding.as_ref().ok_or(anyhow!("Database not open")) {
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

        let db_binding = self.curr_db.borrow();
        let db = match db_binding.as_ref().ok_or(anyhow!("Database not open")) {
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

        let db_binding = self.curr_db.borrow();
        let db = match db_binding.as_ref().ok_or(anyhow!("Database not open")) {
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

impl Handler<StoreMasterPassword> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: StoreMasterPassword, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        self.stored_master_password
            .replace(Some(msg.master_password));

        gui.masterPasswordStored();
    }
}

impl Handler<InvalidateMasterPassword> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, _: InvalidateMasterPassword, _: &mut Self::Context) -> Self::Result {
        self.abort_ongoing_operations();

        let binding = self.gui.clone();
        let binding = binding.pinned();
        let mut gui = binding.borrow_mut();

        if let Some(pw) = self.stored_master_password.take() {
            drop(pw); // SecVec drop impl zeroes out.
        }

        if let Some(mut encrypted_pw) = self.curr_master_pw.take() {
            encrypted_pw.zeroize();
        }

        gui.isMasterPasswordEncrypted = false;
        gui.masterPasswordInvalidated();
        gui.masterPasswordStateChanged(false);
        println!("Master password invalidated.");
    }
}

impl Handler<CheckLockingStatus> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, _: CheckLockingStatus, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow_mut();

        let current_master_pw = self.curr_master_pw.borrow();

        match current_master_pw.as_ref() {
            Some(_) => gui.lockingStatusReceived("set".to_string()),
            None => gui.lockingStatusReceived("unset".to_string()),
        }
    }
}

impl Handler<EncryptMasterPassword> for KeepassRxActor {
    type Result = ();

    // TODO do not bother with this if database locking is disabled in
    // settings.
    fn handle(&mut self, _: EncryptMasterPassword, _: &mut Self::Context) -> Self::Result {
        self.abort_ongoing_operations();
        let stored_pw = self.stored_master_password.take();
        let binding = self.gui.clone();
        let ez_open = self.curr_master_pw.clone();

        // Encrypting the password is CPU-intensive and can block the
        // UI. It is run on a separate fire-and-forget async call,
        // which itself spawns a separate thread pool to actually
        // encrypt the password. If we were to wait for the
        // actix::spawn to complete, the UI would still block. The
        // type annotation here is just for the compiler.
        let handle: JoinHandle<Result<_>> = actix::spawn(async move {
            let stored_pw = stored_pw.ok_or(anyhow!("No master password stored"));

            let (tx, rx) = tokio::sync::oneshot::channel();
            rayon::spawn(move || {
                if let Err(_) = tx.send(stored_pw.map(|pw| EncryptedPassword::new(pw))) {
                    println!("Receiver dropped before receiving encrypted password.");
                }
            });

            // Very nested errors.
            let result = rx
                .await
                .map_err(Box::<dyn std::error::Error>::from)
                .and_then(|res2| res2.map_err(Into::into))
                .and_then(|res3| res3.map_err(Into::into));

            let binding = binding.pinned();
            let mut gui = binding.borrow_mut();

            match result {
                Ok(encrypted_pw) => {
                    ez_open.replace(Some(encrypted_pw));
                    gui.isMasterPasswordEncrypted = true;
                    gui.masterPasswordStateChanged(true);
                    println!("Master password encrypted.");
                }
                Err(err) => {
                    gui.errorReceived(format!("{}", err));
                }
            }

            Ok(())
        });

        self.current_operation = Some(handle);
    }
}

impl Handler<DecryptMasterPassword> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: DecryptMasterPassword, _: &mut Self::Context) -> Self::Result {
        let short_pw = msg.short_password;
        let gui_binding = self.gui.clone();
        let encrypted_master_pw = self.curr_master_pw.clone();
        let stored_master_pw = self.stored_master_password.clone();

        self.abort_ongoing_operations();

        let handle: JoinHandle<Result<_>> = actix::spawn(async move {
            // Remove from RefCell.
            let mut maybe_master_pw = encrypted_master_pw.take();

            // Extract from inner option.
            let master_pw = maybe_master_pw
                .take()
                .ok_or(anyhow!("No master password stored"));

            let gui_binding = gui_binding.pinned();
            let mut gui = gui_binding.borrow_mut();

            let (tx, rx) = tokio::sync::oneshot::channel();
            rayon::spawn(move || {
                if let Err(_) = tx.send(master_pw.map(|pw| pw.decrypt(&short_pw))) {
                    println!("Receiver dropped before receiving decrypted password.");
                }
            });

            // Very nested result
            let result = rx
                .await
                .map_err(Box::<dyn std::error::Error>::from)
                .and_then(|res2| res2.map_err(Into::into))
                .and_then(|res3| res3.map_err(Into::into));

            match result {
                Ok(secure_decrypted_password) => {
                    stored_master_pw.replace(Some(secure_decrypted_password));
                    gui.isMasterPasswordEncrypted = false;
                    gui.masterPasswordStateChanged(false);
                    gui.masterPasswordDecrypted();
                    println!("Master password decrypted.");
                }
                Err(err) => {
                    gui.errorReceived(format!("{}", err));
                }
            }

            Ok(())
        });

        self.current_operation = Some(handle);
    }
}

#[derive(QObject, Default)]
#[allow(non_snake_case)]
pub struct KeepassRx {
    base: qt_base_class!(trait QObject),
    actor: Option<Addr<KeepassRxActor>>,

    databaseOpen: qt_property!(bool),
    isMasterPasswordEncrypted: qt_property!(bool; NOTIFY masterPasswordStateChanged),

    // database management
    setFile: qt_method!(fn(&self, path: String, is_db: bool)),
    openDatabase: qt_method!(fn(&mut self, path: String, key_path: QString)),
    closeDatabase: qt_method!(fn(&mut self)),

    // group and entry management
    getGroups: qt_method!(fn(&self)),
    getEntries: qt_method!(fn(&self, search_term: QString)),
    getTotp: qt_method!(fn(&self, entry_uuid: QString)),

    // easy-open management
    storeMasterPassword: qt_method!(fn(&self, master_password: QString)),
    encryptMasterPassword: qt_method!(fn(&self)),
    decryptMasterPassword: qt_method!(fn(&self, short_password: QString)),
    invalidateMasterPassword: qt_method!(fn(&self)),
    checkLockingStatus: qt_method!(fn(&self)),

    // signals
    fileSet: qt_signal!(path: String),
    databaseOpened: qt_signal!(),
    databaseClosed: qt_signal!(),
    databaseOpenFailed: qt_signal!(message: String),
    groupsReceived: qt_signal!(groups: QStringList),
    entriesReceived: qt_signal!(entries: QVariantMap),
    errorReceived: qt_signal!(error: String),
    totpReceived: qt_signal!(totp: QVariantMap),

    // easy-open signals
    masterPasswordStored: qt_signal!(),
    masterPasswordInvalidated: qt_signal!(),
    masterPasswordStateChanged: qt_signal!(encrypted: bool),
    masterPasswordDecrypted: qt_signal!(),
    lockingStatusReceived: qt_signal!(status: String),
}

#[allow(non_snake_case)]
impl KeepassRx {
    /// Copy a chosen databse file into the local data structure. This
    /// is completely sync because we need to finalize() the transfer
    /// in QML.
    #[with_executor]
    pub fn setFile(&self, path: String, is_db: bool) {
        let path_string = path.to_string();

        let copy_file = move || {
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

            // Nuke db.kdbx if it exists and is a directory for some
            // reason. Can result from corruption or weirdness.
            if dest.exists() && dest.is_dir() {
                println!(
                    "{} is a directory for some reason. Removing.",
                    dest.display()
                );
                remove_dir_all(&dest)?;
            }

            let bytes_copied = std::fs::copy(&source, &dest)?;
            println!("Copied {} bytes", bytes_copied);
            Ok(())
        };

        match copy_file() {
            Ok(_) => self.fileSet(path_string),
            Err(err) => self.databaseOpenFailed(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn openDatabase(&mut self, path: String, key_path: QString) {
        let key_path = match key_path {
            kp if !kp.is_null() && !kp.is_empty() => Some(kp.to_string()),
            _ => None,
        };

        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(OpenDatabase { path, key_path }));
    }

    #[with_executor]
    pub fn closeDatabase(&mut self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(CloseDatabase));
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

    #[with_executor]
    pub fn storeMasterPassword(&self, master_password: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(StoreMasterPassword {
            master_password: SecUtf8::from(master_password.to_string()),
        }));
    }

    #[with_executor]
    pub fn encryptMasterPassword(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(EncryptMasterPassword));
    }

    #[with_executor]
    pub fn decryptMasterPassword(&self, short_password: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(DecryptMasterPassword {
            short_password: SecUtf8::from(short_password.to_string()),
        }));
    }

    #[with_executor]
    pub fn invalidateMasterPassword(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(InvalidateMasterPassword));
    }

    #[with_executor]
    pub fn checkLockingStatus(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(CheckLockingStatus));
    }
}
