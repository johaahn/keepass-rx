use actix::prelude::*;
use anyhow::{Result, anyhow};
use keepass::{Database, DatabaseKey};
use qmetaobject::*;
use secstr::SecUtf8;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::sync::Arc;
use tokio::task::{JoinHandle, spawn_blocking};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use super::KeepassRx;
use super::models::RxListItem;
use crate::{
    gui::utils::imported_databases_path,
    rx::{EncryptedPassword, RxDatabase, RxFieldName, ZeroableDatabase},
};

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
pub struct OpenDatabase {
    pub db_name: String,
    pub key_path: Option<String>,
}

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
pub struct DeleteDatabase {
    pub db_name: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct CloseDatabase;

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetEntries {
    // None = root uuid
    pub group_uuid: Option<Uuid>,
    pub search_term: Option<String>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetSingleEntry {
    pub entry_uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetFieldValue {
    pub entry_uuid: Uuid,
    pub field_name: RxFieldName,
}

impl GetEntries {
    pub fn root(search_term: Option<String>) -> Self {
        Self {
            group_uuid: None,
            search_term,
        }
    }

    pub fn for_uuid(group_uuid: Uuid, search_term: Option<String>) -> Self {
        Self {
            group_uuid: Some(group_uuid),
            search_term,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetGroups {
    // None = root uuid
    pub group_uuid: Option<Uuid>,
}

impl GetGroups {
    pub fn root() -> Self {
        Self { group_uuid: None }
    }

    pub fn for_uuid(group_uuid: Uuid) -> Self {
        Self {
            group_uuid: Some(group_uuid),
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetTotp {
    pub entry_uuid: String,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StoreMasterPassword {
    pub master_password: SecUtf8,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct EncryptMasterPassword;

#[derive(Message)]
#[rtype(result = "()")]
pub struct DecryptMasterPassword {
    pub short_password: SecUtf8,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct InvalidateMasterPassword;

#[derive(Message)]
#[rtype(result = "()")]
pub struct CheckLockingStatus;

impl Handler<OpenDatabase> for KeepassRxActor {
    type Result = AtomicResponse<Self, anyhow::Result<()>>;
    fn handle(&mut self, msg: OpenDatabase, _: &mut Self::Context) -> Self::Result {
        // Opening the database is synchronous I/O, which means it
        // must be done on a separate thread.
        let db_path = imported_databases_path().join(msg.db_name);

        // Clone here so we can encrypt later.
        let stored_pw = self.stored_master_password.clone();

        println!("Opening DB: {}", db_path.display());

        AtomicResponse::new(Box::pin(
            async move {
                let pw_binding = stored_pw.borrow();
                let pw_binding = pw_binding.as_ref();
                let pw_binding = pw_binding
                    .as_deref()
                    .ok_or(anyhow!("[OpenDB] No master password stored"))?;

                let mut db_file = File::open(db_path)?;
                let key_file = msg.key_path.map(|p| File::open(p));

                let db_key = DatabaseKey::new().with_password(pw_binding.unsecure());
                let db_key = match key_file {
                    // the double ? coerces File::open result and with_keyfile result.
                    Some(file) => db_key.with_keyfile(&mut file?)?,
                    None => db_key,
                };

                let open_result = spawn_blocking(move || -> Result<Database> {
                    let db = Database::open(&mut db_file, db_key)?;
                    Ok(db)
                })
                .await??;

                Ok(open_result)
            }
            .into_actor(self)
            .map(|result: Result<Database>, this, _| {
                let binding = this.gui.clone();
                let binding = binding.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(keepass_db) => {
                        let wrapped_db = Zeroizing::new(ZeroableDatabase(keepass_db));
                        let rx_db = RxDatabase::new(wrapped_db);
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
                let mut db = db.ok_or(anyhow!("CloseDatabase: Database not open"))?;
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

impl Handler<DeleteDatabase> for KeepassRxActor {
    type Result = Result<()>;

    fn handle(&mut self, msg: DeleteDatabase, _: &mut Self::Context) -> Self::Result {
        println!("Deleting db {}", msg.db_name);
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_path = imported_databases_path().join(&msg.db_name);

        match std::fs::remove_file(&db_path) {
            Ok(_) => gui.databaseDeleted(QString::from(msg.db_name)),
            Err(err) => gui.errorReceived(format!("{}", err)),
        }

        Ok(())
    }
}

impl Handler<GetGroups> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, msg: GetGroups, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_binding = self.curr_db.borrow();
        let db = match db_binding
            .as_ref()
            .ok_or(anyhow!("GetGroups: Database not open"))
        {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let group_uuid = match msg.group_uuid {
            Some(id) => id,
            None => db.root_group().uuid,
        };

        let group = db.get_group(group_uuid);

        let this_group_name = group
            .as_ref()
            .map(|grp| QString::from(grp.name.as_ref()))
            .unwrap_or_default();

        let parent_group_uuid = group
            .as_ref()
            .and_then(|grp| grp.parent.map(|parent| QString::from(parent.to_string())))
            .unwrap_or_default();

        let this_group_uuid = QString::from(group_uuid.to_string());
        gui.groupsReceived(parent_group_uuid, this_group_uuid, this_group_name);
    }
}

impl Handler<GetEntries> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetEntries, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_binding = self.curr_db.borrow();
        let db = match db_binding
            .as_ref()
            .ok_or(anyhow!("GetEntries: Database not open"))
        {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let group_uuid = match msg.group_uuid {
            Some(id) => id,
            None => db.root_group().uuid,
        };

        let search_term = msg.search_term.as_deref();
        let group = db.get_group_filter_subgroups(group_uuid, search_term);
        let entries = db.get_entries(group_uuid, search_term);

        // Groups first, then entries below.
        let mut item_list: Vec<RxListItem> = group
            .map(|grp| grp.subgroups.into_iter().map(RxListItem::from).collect())
            .unwrap_or_default();

        item_list.append(&mut entries.into_iter().map(RxListItem::from).collect());

        let q_entries: QVariantList = item_list.into_iter().collect();
        gui.entriesReceived(q_entries);
    }
}

impl Handler<GetSingleEntry> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetSingleEntry, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_binding = self.curr_db.borrow();
        let db = match db_binding
            .as_ref()
            .ok_or(anyhow!("GetSingleEntry: Database not open"))
        {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let entry = db.get_entry(msg.entry_uuid);

        let q_entry = match entry.map(QVariantMap::from) {
            Some(map) => map.to_qvariant(),
            None => QVariant::default(), // null
        };

        gui.singleEntryReceived(q_entry);
    }
}

impl Handler<GetFieldValue> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetFieldValue, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_binding = self.curr_db.borrow();
        let db = match db_binding
            .as_ref()
            .ok_or(anyhow!("GetSingleEntry: Database not open"))
        {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let value: QString = db
            .get_entry(msg.entry_uuid)
            .and_then(|entry| {
                entry
                    .get_field_value(&msg.field_name)
                    .map(|val| QString::from(val))
            })
            .unwrap_or_default();

        gui.fieldValueReceived(
            QString::from(msg.entry_uuid.to_string()),
            QString::from(msg.field_name.to_string()),
            value,
        );
    }
}

impl Handler<GetTotp> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetTotp, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.clone();
        let binding = binding.pinned();
        let gui = binding.borrow();

        let db_binding = self.curr_db.borrow();
        let db = match db_binding
            .as_ref()
            .ok_or(anyhow!("GetTotp: Database not open"))
        {
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
        // actix::spawn to complete, the UI would still block.
        let handle: JoinHandle<Result<_>> = actix::spawn(async move {
            let stored_pw = stored_pw.ok_or(anyhow!("[Encrypt] No master password stored"));

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

            // In case we need to retain it because of error.
            let backup = maybe_master_pw.clone();

            // Extract from inner option.
            let master_pw = maybe_master_pw
                .take()
                .ok_or(anyhow!("[Decrypt] No master password stored"));

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
                    // Here, we put the encrypted pw back into the secret.
                    encrypted_master_pw.replace(backup);
                    gui.decryptionFailed(QString::from(format!("{}", err)));
                }
            }

            Ok(())
        });

        self.current_operation = Some(handle);
    }
}
