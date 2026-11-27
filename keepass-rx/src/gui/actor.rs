use actix::prelude::*;
use anyhow::{Result, anyhow};
use keepass::error::{CryptographyError, DatabaseKeyError, DatabaseOpenError};
use keepass::{Database, DatabaseKey};
use libsodium_rs::utils::SecureVec;
use log::{debug, error, info, warn};
use qmetaobject::{QMetaType, QStringList, QVariantMap, prelude::*};
use secstr::SecUtf8;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::File;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;
use tokio::task::AbortHandle;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use super::{KeepassRx, RxDbType};
use crate::app::{AppState, KeyFile};
#[cfg(feature = "sailfish")]
use crate::crypto::sailfish_secrets;
use crate::crypto::{EncryptedPassword, IncorrectShortCode, MasterKey};
use crate::gui::save_auth::{
    AuthPurpose, SaveAuthMethod, SaveAuthorizer, SystemAuthUnavailable, default_authorizer,
};
use crate::gui::utils::{app_data_path, synced_databases_path};
#[cfg(feature = "sailfish")]
use crate::gui::{
    polkit_auth::lock_code_missing, qml::last_db_path, settings::database_locking,
};
use crate::rx::virtual_hierarchy::{
    AllTags, AllTemplates, DefaultView, SavedSearches, TotpEntries, VirtualHierarchy,
    VirtualHierarchyType,
};
use crate::{
    gui::{RxViewMode, utils::imported_databases_path},
    rx::{
        RxDatabase, RxDbFormat, RxEdit, RxEntryEdit, RxFieldName, RxSourceFile, SaveError,
        SaveOutcome,
        closing::{APP_CLOSING, StopWhenClosing},
        edit_values_to_map,
        open::{build_rx_database, read_database},
        save_database,
    },
};

fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// KDBX 3 decrypts before checking the key, so a wrong key fails as `InvalidPadding`.
fn is_credentials_failure(err: &DatabaseOpenError) -> bool {
    matches!(
        err,
        DatabaseOpenError::Key(DatabaseKeyError::IncorrectKey)
            | DatabaseOpenError::Cryptography(CryptographyError::InvalidPadding(_))
    )
}

fn is_credentials_error(err: &anyhow::Error) -> bool {
    err.downcast_ref().is_some_and(is_credentials_failure)
}

pub struct KeepassRxActor {
    app_state: Rc<QObjectBox<AppState>>,
    gui: Rc<QObjectBox<KeepassRx>>,
    //curr_db: RefCell<Option<Zeroizing<RxDatabase>>>,
    curr_master_pw: Rc<RefCell<Option<EncryptedPassword>>>,
    stored_master_password: Rc<RefCell<Option<SecUtf8>>>,

    // any in-progress operation on another thread pool that might
    // need to be aborted.
    current_operation: Option<AbortHandle>,

    saving: bool,
    wrong_codes: Cell<u8>,
    deferred: Vec<DeferredLock>,
    save_authorizer: Box<dyn SaveAuthorizer>,
}

impl Default for KeepassRxActor {
    fn default() -> Self {
        Self {
            app_state: Default::default(),
            gui: Default::default(),
            curr_master_pw: Default::default(),
            stored_master_password: Default::default(),
            current_operation: None,
            saving: false,
            wrong_codes: Cell::new(0),
            deferred: Vec::new(),
            save_authorizer: default_authorizer(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeferredLock {
    Encrypt,
    Invalidate,
    Close,
}

struct SavePlan {
    source: RxSourceFile,
    key: DatabaseKey,
    backup: PathBuf,
}

const MAX_WRONG_CODES: u8 = 3;

enum WrongCode {
    Left(i32),
    LockedOut,
}

fn no_unlock_code() -> anyhow::Error {
    anyhow!("No unlock code is set up for this database yet. Try again.")
}

#[cfg(not(feature = "sailfish"))]
fn reveal_master_password(
    encrypted: Option<&EncryptedPassword>,
    credential: SaveCredential,
) -> Result<SecUtf8> {
    let encrypted = encrypted.ok_or_else(no_unlock_code)?;
    match credential {
        SaveCredential::ShortCode(code) => encrypted.reveal(code),
        SaveCredential::System => Err(IncorrectShortCode.into()),
    }
}

#[cfg(feature = "sailfish")]
fn reveal_master_password(
    encrypted: Option<&EncryptedPassword>,
    credential: SaveCredential,
) -> Result<SecUtf8> {
    match credential {
        SaveCredential::Password(password) => Ok(password),
        SaveCredential::System => {
            encrypted
                .ok_or_else(no_unlock_code)?
                .reveal()
                .map_err(|err| {
                    warn!("Could not unlock with the key from Sailfish Secrets: {err}");
                    if err.downcast_ref::<IncorrectShortCode>().is_some() {
                        err
                    } else {
                        SystemAuthUnavailable.into()
                    }
                })
        }
    }
}

fn build_database_key(
    password: &SecUtf8,
    has_key_file: bool,
    key_file_bytes: Result<Option<SecureVec<u8>>>,
) -> Result<DatabaseKey> {
    let db_key = DatabaseKey::new().with_password(password.unsecure());
    if !has_key_file {
        return Ok(db_key);
    }

    match key_file_bytes? {
        Some(bytes) => {
            debug!("Using a key file of {} bytes.", bytes.len());
            let mut reader: &[u8] = &bytes;
            Ok(db_key.with_keyfile(&mut reader)?)
        }
        None => Ok(db_key),
    }
}

fn backup_path_for(source: &RxSourceFile) -> Result<PathBuf> {
    let file_name = source
        .path
        .file_name()
        .ok_or(anyhow!("Database path has no file name"))?
        .to_string_lossy();
    let kind = if source.path.starts_with(synced_databases_path()) {
        "synced"
    } else {
        "imported"
    };

    Ok(app_data_path()
        .join("backups")
        .join(kind)
        .join(format!("{file_name}.bak")))
}

fn view_for_mode(mode: RxViewMode, db: &RxDatabase) -> VirtualHierarchyType {
    match mode {
        RxViewMode::All => VirtualHierarchyType::DefaultView(DefaultView::new(db)),
        RxViewMode::Templates => VirtualHierarchyType::AllTemplates(AllTemplates::new(db)),
        RxViewMode::Totp => VirtualHierarchyType::TotpEntries(TotpEntries::new(db)),
        RxViewMode::Tags => VirtualHierarchyType::AllTags(AllTags::new(db)),
        RxViewMode::SavedSearches => {
            VirtualHierarchyType::SavedSearches(SavedSearches::new(db))
        }
    }
}

impl KeepassRxActor {
    pub fn new(gui: &Rc<QObjectBox<KeepassRx>>, app_state: &Rc<QObjectBox<AppState>>) -> Self {
        let actor = Self {
            gui: gui.clone(),
            app_state: app_state.clone(),
            ..Default::default()
        };
        #[cfg(feature = "sailfish")]
        actor.check_system_unlock();
        #[cfg(feature = "sailfish")]
        actor.restore_master_password();
        gui.pinned().borrow_mut().saveAuthMethod =
            QString::from(actor.save_authorizer.method().as_str());
        actor
    }

    #[cfg(feature = "sailfish")]
    fn restore_master_password(&self) {
        if !database_locking() {
            EncryptedPassword::forget_saved();
            return;
        }
        let database = last_db_path().unwrap_or_default();
        match EncryptedPassword::load(&database) {
            Ok(Some(encrypted_pw)) => {
                self.curr_master_pw.replace(Some(encrypted_pw));
                self.save_authorizer.remember();
                let binding = self.gui.pinned();
                let mut gui = binding.borrow_mut();
                gui.isMasterPasswordEncrypted = true;
                gui.guiState = super::RxGuiState::Locked;
                info!("Master password restored.");
            }
            Ok(None) => {}
            Err(err) => {
                warn!("Could not restore the master password: {err}");
                EncryptedPassword::forget_saved();
            }
        }
    }

    pub fn abort_ongoing_operations(&self) {
        // Abort any in-progress encryption operation.
        if let Some(ref in_progress) = self.current_operation {
            if !in_progress.is_finished() {
                in_progress.abort();
                info!("Aborting ongoing encryption/decryption operation.");
            }
        }
    }

    /// Extract key file bytes, if they exist.
    pub fn key_file_bytes(&self) -> Result<Option<SecureVec<u8>>> {
        let app_state = self.app_state.pinned();
        let app_state = app_state.borrow();
        let key_file = app_state.db_key_ref();
        let master_key = app_state.master_key();

        match master_key {
            Some(mk) => key_file.map(|kf| kf.bytes(mk)).transpose(),
            _ => Ok(key_file.and_then(|kf| kf.bytes_unencrypted())),
        }
    }

    fn defer(&mut self, action: DeferredLock) {
        if !self.deferred.contains(&action) {
            self.deferred.push(action);
        }
    }

    fn release_master_password(&self, credential: SaveCredential) -> Result<SecUtf8> {
        let revealed =
            reveal_master_password(self.curr_master_pw.borrow().as_ref(), credential)?;
        self.wrong_codes.set(0);
        Ok(revealed)
    }

    fn wrong_code_entered(&self, ctx: &mut Context<Self>) -> WrongCode {
        let wrong = self.wrong_codes.get() + 1;
        if wrong < MAX_WRONG_CODES {
            self.wrong_codes.set(wrong);
            return WrongCode::Left(i32::from(MAX_WRONG_CODES - wrong));
        }

        self.wrong_codes.set(0);
        self.save_authorizer.forget();
        if let Some(mut encrypted_pw) = self.curr_master_pw.take() {
            encrypted_pw.zeroize();
        }
        ctx.notify(InvalidateMasterPassword);
        if self.app_state.pinned().borrow().curr_db_ref().is_ok() {
            ctx.notify(CloseDatabase);
        }
        WrongCode::LockedOut
    }

    fn prepare_save(&self, credential: SaveCredential) -> Result<SavePlan> {
        if self.saving {
            return Err(anyhow!("A save is already in progress"));
        }

        let source = {
            let app_state = self.app_state.pinned();
            let app_state = app_state.borrow();
            app_state
                .curr_db_ref()?
                .source()
                .cloned()
                .ok_or(anyhow!("This database cannot be saved"))?
        };

        let has_key_file = self.app_state.pinned().borrow().db_key_ref().is_some();
        let password = self.release_master_password(credential)?;
        let key = build_database_key(&password, has_key_file, self.key_file_bytes())?;
        drop(password);
        let backup = backup_path_for(&source)?;

        Ok(SavePlan {
            source,
            key,
            backup,
        })
    }

    fn finish_save(&mut self, ctx: &mut Context<Self>) {
        self.saving = false;
        {
            let binding = self.gui.pinned();
            let mut gui = binding.borrow_mut();
            gui.saving = false;
            gui.savingChanged();
        }

        for action in std::mem::take(&mut self.deferred) {
            match action {
                DeferredLock::Encrypt => ctx.notify(EncryptMasterPassword),
                DeferredLock::Invalidate => ctx.notify(InvalidateMasterPassword),
                DeferredLock::Close => ctx.notify(CloseDatabase),
            }
        }
    }

    fn replace_database(&self, outcome: SaveOutcome, source: RxSourceFile) -> Result<()> {
        let source = RxSourceFile {
            sha256: outcome.sha256,
            ..source
        };
        let rx_db = build_rx_database(outcome.db, source)?;

        let binding = self.gui.pinned();
        let mut gui = binding.borrow_mut();
        let app_state = self.app_state.pinned();
        let mut app_state = app_state.borrow_mut();

        let view = view_for_mode(gui.viewMode, &rx_db);
        gui.metadata = rx_db.metadata().into();

        if let Ok(mut old) = app_state.take_db() {
            old.close();
        }
        app_state.set_db(Zeroizing::new(rx_db));
        app_state.set_curr_view(view);
        Ok(())
    }

    /// Common pattern: borrow gui + app_state, get curr_db, call `f`,
    /// or forward the error to the GUI.
    fn with_db<F: FnOnce(&KeepassRx, &RxDatabase)>(&self, f: F) {
        let gui_binding = self.gui.pinned();
        let gui = gui_binding.borrow();
        let app_state_binding = self.app_state.pinned();
        let app_state = app_state_binding.borrow();
        let db = match app_state.curr_db_ref() {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };
        f(&gui, &db);
    }
}

impl Supervised for KeepassRxActor {}

impl SystemService for KeepassRxActor {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        debug!("service started");
        self.gui.pinned().borrow_mut().actor = Some(ctx.address());
    }
}

impl Actor for KeepassRxActor {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.gui.pinned().borrow_mut().actor = Some(ctx.address());
    }
}

impl KeepassRxActor {
    #[cfg(feature = "sailfish")]
    fn check_system_unlock(&self) {
        let available = if lock_code_missing() {
            info!("No lock code is set, so Authenticate stays hidden.");
            false
        } else {
            match sailfish_secrets::probe() {
                Ok(()) => {
                    info!("Sailfish Secrets has the plugins the unlock key needs.");
                    true
                }
                Err(err) => {
                    warn!("{err:#}");
                    false
                }
            }
        };
        self.save_authorizer.set_available(available);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetViewMode(pub RxViewMode);

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
pub struct OpenDatabase {
    pub db_name: String,
    pub db_type: RxDbType,
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
pub struct GetMetadata;

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetEntries {
    // None = root uuid
    pub container_uuid: Option<Uuid>,
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
    pub purpose: String,
}

impl GetEntries {
    pub fn for_uuid(group_uuid: Uuid, search_term: Option<String>) -> Self {
        Self {
            container_uuid: Some(group_uuid),
            search_term,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetEntryForEdit {
    pub entry_uuid: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct SaveEntry {
    pub edit: RxEntryEdit,
    pub credential: SaveCredential,
}

pub enum SaveCredential {
    System,
    #[cfg(not(feature = "sailfish"))]
    ShortCode(SecUtf8),
    #[cfg(feature = "sailfish")]
    Password(SecUtf8),
}

impl SaveCredential {
    pub fn from_input(input: &str) -> Self {
        if input.is_empty() {
            return Self::System;
        }
        Self::typed(SecUtf8::from(input))
    }

    #[cfg(not(feature = "sailfish"))]
    fn typed(secret: SecUtf8) -> Self {
        Self::ShortCode(secret)
    }

    #[cfg(feature = "sailfish")]
    fn typed(secret: SecUtf8) -> Self {
        Self::Password(secret)
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct GetContainer {
    // None = root uuid
    pub container_uuid: Option<Uuid>,
}

impl GetContainer {
    pub fn root() -> Self {
        Self {
            container_uuid: None,
        }
    }

    pub fn for_uuid(group_uuid: Uuid) -> Self {
        Self {
            container_uuid: Some(group_uuid),
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

impl DecryptMasterPassword {
    #[cfg(not(feature = "sailfish"))]
    fn decrypter(self) -> impl FnOnce(EncryptedPassword) -> Result<SecUtf8> + Send + 'static {
        move |encrypted| encrypted.decrypt(self.short_password)
    }

    #[cfg(feature = "sailfish")]
    fn decrypter(self) -> impl FnOnce(EncryptedPassword) -> Result<SecUtf8> + Send + 'static {
        EncryptedPassword::decrypt
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct InvalidateMasterPassword;

#[derive(Message)]
#[rtype(result = "()")]
pub struct CheckLockingStatus;

impl Handler<SetViewMode> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, msg: SetViewMode, _: &mut Self::Context) -> Self::Result {
        let SetViewMode(mode) = msg;

        let binding = self.gui.pinned();
        let mut gui = binding.borrow_mut();

        let app_state = self.app_state.pinned();
        let mut app_state = app_state.borrow_mut();
        let db_binding = app_state.curr_db_ref();

        let db = match db_binding {
            Ok(db) => db,
            Err(err) => return gui.errorReceived(format!("{}", err)),
        };

        let view = view_for_mode(mode, &db);

        app_state.set_curr_view(view);
        gui.viewMode = mode;
        gui.viewModeChanged(mode);

        debug!("Set view to: {}", app_state.curr_view_ref().unwrap().name());
    }
}

impl Handler<OpenDatabase> for KeepassRxActor {
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;
    fn handle(&mut self, msg: OpenDatabase, _: &mut Self::Context) -> Self::Result {
        // Determine if key file is set and extract key bytes if needed.
        let app_state = self.app_state.pinned();
        let app_state = app_state.borrow();
        let has_key_file = app_state.db_key_ref().is_some();

        let db_path = match msg.db_type {
            RxDbType::Imported => imported_databases_path().join(msg.db_name),
            RxDbType::Synced => synced_databases_path().join(msg.db_name),
        };

        // Clone here so we can encrypt later.
        let stored_pw = self.stored_master_password.clone();
        let maybe_key_file_bytes = self.key_file_bytes();

        info!("Opening {} DB: {}", msg.db_type, db_path.display());

        Box::pin(
            async move {
                let pw_binding = stored_pw.borrow();
                let pw_binding = pw_binding.as_ref();
                let pw_binding = pw_binding
                    .as_deref()
                    .ok_or(anyhow!("[OpenDB] No master password stored"))?;

                let db_key = build_database_key(pw_binding, has_key_file, maybe_key_file_bytes)?;

                // Opening the database is synchronous I/O, which means it
                // must be done on a separate thread.
                let open_result =
                    actix_rt::task::spawn_blocking(move || -> Result<_> {
                        read_database(&db_path, db_key)
                    })
                    .await
                    .map_err(|err| anyhow!(err))??;

                Ok(open_result)
            }
            .into_actor(self)
            .map(|result: Result<(Database, RxSourceFile)>, this, _| {
                let binding = this.gui.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok((keepass_db, source)) => {
                        let conversion = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
                            let rx_db = build_rx_database(keepass_db, source)?;
                            let view = VirtualHierarchyType::DefaultView(DefaultView::new(&rx_db));
                            Ok((rx_db, view))
                        }));

                        let (rx_db, view) = match conversion {
                            Ok(Ok(value)) => value,
                            Ok(Err(err)) => {
                                error!("Database materialization failed: {err:?}");
                                gui.databaseOpenFailed(format!(
                                    "Unable to load database contents: {err}. See log file."
                                ), false);
                                return Ok(());
                            }
                            Err(payload) => {
                                let payload = panic_payload_to_string(payload.as_ref());
                                let backtrace = std::backtrace::Backtrace::force_capture();
                                error!(
                                    "Panic while materializing database: {payload}\n{backtrace}"
                                );
                                gui.databaseOpenFailed(format!(
                                    "Internal error while loading database: {payload}. See log file."
                                ), false);
                                return Ok(());
                            }
                        };

                        let app_state = this.app_state.pinned();
                        let mut app_state = app_state.borrow_mut();

                        // Encrypt key file
                        if let Some(kf) = app_state.take_db_key() {
                            if kf.is_unencrypted() {
                                let mk = MasterKey::new()
                                    .expect("Could not create key file master key");
                                let kf = kf.encrypt(&mk).expect("Could not encrypt key file");

                                app_state.set_master_key(Some(mk));
                                app_state.set_db_key(Some(kf));
                            } else {
                                app_state.set_db_key(Some(kf));
                            }
                        }

                        gui.rootGroupUuid = QString::from(rx_db.root_group().uuid.to_string());
                        gui.metadata = rx_db.metadata().into();

                        let can_edit = rx_db
                            .source()
                            .is_some_and(|source| source.format == RxDbFormat::Kdbx4);
                        app_state.set_db(Zeroizing::new(rx_db));
                        app_state.set_curr_view(view);

                        gui.databaseOpen = true;
                        gui.canEdit = can_edit;
                        gui.canEditChanged();
                        gui.databaseOpened();

                        gui.viewMode = RxViewMode::All;
                        gui.viewModeChanged(RxViewMode::All);
                    }
                    Err(err) => {
                        error!("Database open failed: {err:?}");
                        gui.databaseOpenFailed(format!("{}", err), is_credentials_error(&err));
                    }
                }

                Ok(())
            }),
        )
    }
}

impl Handler<CloseDatabase> for KeepassRxActor {
    type Result = AtomicResponse<Self, ()>;

    fn handle(&mut self, _: CloseDatabase, _: &mut Self::Context) -> Self::Result {
        if self.saving {
            self.defer(DeferredLock::Close);
            return AtomicResponse::new(Box::pin(async {}.into_actor(self)));
        }

        // Remove from cell
        let app_state = self.app_state.pinned();
        let mut app_state = app_state.borrow_mut();
        let db = app_state.take_db();

        AtomicResponse::new(Box::pin(
            async move {
                // Remove from option.
                let mut db = db?;
                db.close();
                Ok(())
            }
            .into_actor(self)
            .map(|result: Result<()>, this, _| {
                let binding = this.gui.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(_) => {
                        gui.databaseOpen = false;
                        gui.canEdit = false;
                        gui.canEditChanged();
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
        info!("Deleting db {}", msg.db_name);
        let binding = self.gui.pinned();
        let gui = binding.borrow();

        let db_path = imported_databases_path().join(&msg.db_name);

        match std::fs::remove_file(&db_path) {
            Ok(_) => gui.databaseDeleted(QString::from(msg.db_name)),
            Err(err) => gui.errorReceived(format!("{}", err)),
        }

        Ok(())
    }
}

impl Handler<GetMetadata> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, _: GetMetadata, _: &mut Self::Context) -> Self::Result {
        self.with_db(|gui, db| {
            gui.metadataReceived(QVariantMap::from(db.metadata()));
        });
    }
}

impl Handler<GetContainer> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, msg: GetContainer, _: &mut Self::Context) -> Self::Result {
        let app_state = self.app_state.pinned();
        let app_state = app_state.borrow();

        let binding = self.gui.pinned();
        let gui = binding.borrow();

        let view = app_state.curr_view_ref().expect("GetGroup: No view set.");

        let container_uuid = match msg.container_uuid {
            Some(id) => id,
            None => view.root().uuid(),
        };

        let maybe_container = view
            .root()
            .get_container(container_uuid)
            .expect("GetContainer: No container found")
            .contained_ref();

        let this_container_name = maybe_container
            .map(|container| QString::from(container.name().as_ref()))
            .unwrap_or_default();

        let this_container_uuid = QString::from(container_uuid.to_string());
        gui.containerReceived(this_container_uuid, this_container_name);
    }
}

impl Handler<GetEntries> for KeepassRxActor {
    type Result = ();

    fn handle(&mut self, msg: GetEntries, _: &mut Self::Context) -> Self::Result {
        let app_state = self.app_state.pinned();
        let app_state = app_state.borrow();

        let search_type = app_state.search_type();

        let binding = self.gui.pinned();
        let gui = binding.borrow();

        let search_term = msg
            .search_term
            .as_deref()
            .map(|term| term.trim())
            .filter(|term| !term.is_empty());
        let viewable = app_state
            .curr_view_ref()
            .expect("GetEntries: Viewable not set.");

        let container_uuid = match msg.container_uuid {
            Some(id) => id,
            None => viewable.root().uuid(),
        };

        let results: QStringList = viewable
            .search(search_type, container_uuid, search_term)
            .into_iter()
            .map(|container| container.uuid().to_string())
            .collect();

        gui.entriesReceived(results);
    }
}

impl Handler<GetSingleEntry> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetSingleEntry, _: &mut Self::Context) -> Self::Result {
        self.with_db(|gui, db| {
            let entry = db.get_entry(msg.entry_uuid);

            let q_entry = match entry.as_deref().map(QVariantMap::from) {
                Some(map) => map.to_qvariant(),
                None => QVariant::default(), // null
            };

            gui.singleEntryReceived(q_entry);
        });
    }
}

impl Handler<GetEntryForEdit> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetEntryForEdit, _: &mut Self::Context) -> Self::Result {
        self.with_db(|gui, db| match db.get_entry(msg.entry_uuid) {
            Some(entry) => gui.entryForEditReceived(edit_values_to_map(&entry.edit_values())),
            None => gui.errorReceived(format!("Entry {} not found", msg.entry_uuid)),
        });
    }
}

impl KeepassRxActor {
    fn save_failed(&self, err: &anyhow::Error, ctx: &mut Context<Self>) {
        let gui = self.gui.pinned();
        let gui = gui.borrow();
        if err.downcast_ref::<IncorrectShortCode>().is_some() {
            match self.wrong_code_entered(ctx) {
                WrongCode::Left(left) => gui.entrySaveAuthRejected(left),
                WrongCode::LockedOut => gui.codeLimitReached(),
            }
        } else if err.downcast_ref::<SystemAuthUnavailable>().is_some() {
            gui.entrySaveCodeRequired();
        } else {
            gui.entrySaveFailed(QString::from(err.to_string()), false);
        }
    }
}

impl Handler<SaveEntry> for KeepassRxActor {
    type Result = ResponseActFuture<Self, ()>;
    fn handle(&mut self, msg: SaveEntry, ctx: &mut Self::Context) -> Self::Result {
        if matches!(msg.credential, SaveCredential::System)
            && let Some(confirmation) = self.save_authorizer.confirm(AuthPurpose::Save)
        {
            return Box::pin(confirmation.into_actor(self).map(move |result, this, ctx| {
                match result {
                    Ok(()) => {
                        let saving = this.begin_save(msg, ctx);
                        ctx.spawn(saving);
                    }
                    Err(err) => {
                        warn!("System authorization failed: {err}");
                        this.gui.pinned().borrow().entrySaveCodeRequired();
                    }
                }
            }));
        }

        self.begin_save(msg, ctx)
    }
}

impl KeepassRxActor {
    fn begin_save(
        &mut self,
        msg: SaveEntry,
        ctx: &mut Context<Self>,
    ) -> ResponseActFuture<Self, ()> {
        let uuid = msg.edit.uuid;
        let plan = match self.prepare_save(msg.credential) {
            Ok(plan) => plan,
            Err(err) => {
                self.save_failed(&err, ctx);
                return Box::pin(async {}.into_actor(self));
            }
        };

        self.saving = true;
        {
            let binding = self.gui.pinned();
            let mut gui = binding.borrow_mut();
            gui.saving = true;
            gui.savingChanged();
        }

        let source = plan.source.clone();
        let edit = RxEdit::UpdateEntry(msg.edit);
        info!("Saving entry {uuid}");

        Box::pin(
            async move {
                actix_rt::task::spawn_blocking(move || {
                    save_database(
                        &plan.source,
                        plan.key,
                        &plan.backup,
                        |db| edit.apply(db).map_err(Into::into),
                        |db| edit.verify(db).map_err(Into::into),
                        &StopWhenClosing(&APP_CLOSING),
                    )
                })
                .await
                .map_err(|err| SaveError::Edit(err.to_string()))?
            }
            .into_actor(self)
            .map(move |result: Result<SaveOutcome, SaveError>, this, ctx| {
                this.finish_save(ctx);

                let outcome = match result {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        error!("Entry save failed: {err}");
                        let gui = this.gui.pinned();
                        let gui = gui.borrow();
                        return match err {
                            SaveError::Open(ref open)
                                if this.save_authorizer.method() != SaveAuthMethod::ShortCode
                                    && is_credentials_failure(open) =>
                            {
                                gui.entrySavePasswordRejected()
                            }
                            _ => gui.entrySaveFailed(
                                QString::from(err.to_string()),
                                matches!(err, SaveError::Conflict),
                            ),
                        };
                    }
                };

                let conflict_name = outcome
                    .conflict_copy
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| QString::from(name.to_string_lossy().as_ref()));
                let gui = this.gui.pinned();
                match this.replace_database(outcome, source) {
                    Ok(()) => {
                        if let Some(name) = conflict_name {
                            gui.borrow().entrySavedWithConflictCopy(name);
                        }
                        gui.borrow().entrySaved(QString::from(uuid.to_string()));
                    }
                    Err(err) => {
                        error!("Reloading saved database failed: {err:?}");
                        gui.borrow().entrySaveFailed(
                            QString::from(format!(
                                "Saved, but the database could not be reloaded: {err}. Reopen it."
                            )),
                            false,
                        );
                    }
                }
            }),
        )
    }
}

fn get_value(db: &RxDatabase, entry_uuid: Uuid, field_name: &RxFieldName) -> QString {
    db.get_entry(entry_uuid)
        .and_then(|entry| {
            entry.get_field_value(&field_name).map(|val| {
                val.value()
                    .map(|s| QString::from(s.as_str()))
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default()
}

impl Handler<GetFieldValue> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetFieldValue, _: &mut Self::Context) -> Self::Result {
        self.with_db(|gui, db| {
            let value = get_value(db, msg.entry_uuid, &msg.field_name);

            gui.fieldValueReceived(
                QString::from(msg.entry_uuid.to_string()),
                QString::from(msg.field_name.to_string()),
                value,
                QString::from(msg.purpose),
            );
        });
    }
}

impl Handler<GetTotp> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: GetTotp, _: &mut Self::Context) -> Self::Result {
        self.with_db(|gui, db| {
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
        });
    }
}

impl Handler<StoreMasterPassword> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, msg: StoreMasterPassword, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.pinned();
        let gui = binding.borrow();

        self.stored_master_password
            .replace(Some(msg.master_password));

        gui.masterPasswordStored();
    }
}

impl Handler<InvalidateMasterPassword> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, _: InvalidateMasterPassword, _: &mut Self::Context) -> Self::Result {
        if self.saving {
            return self.defer(DeferredLock::Invalidate);
        }

        self.abort_ongoing_operations();
        self.save_authorizer.forget();

        let binding = self.gui.pinned();
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
        info!("Master password invalidated.");
    }
}

impl Handler<CheckLockingStatus> for KeepassRxActor {
    type Result = ();
    fn handle(&mut self, _: CheckLockingStatus, _: &mut Self::Context) -> Self::Result {
        let binding = self.gui.pinned();
        let gui = binding.borrow_mut();

        let current_master_pw = self.curr_master_pw.borrow();

        match current_master_pw.as_ref() {
            Some(_) => gui.lockingStatusReceived("set".to_string()),
            None => gui.lockingStatusReceived("unset".to_string()),
        }
    }
}

impl Handler<EncryptMasterPassword> for KeepassRxActor {
    type Result = ResponseActFuture<Self, ()>;

    // TODO do not bother with this if database locking is disabled in
    // settings.
    fn handle(&mut self, _: EncryptMasterPassword, _: &mut Self::Context) -> Self::Result {
        if self.saving {
            self.defer(DeferredLock::Encrypt);
            return Box::pin(async {}.into_actor(self));
        }

        self.abort_ongoing_operations();
        let stored_pw = self.stored_master_password.take();
        #[cfg(feature = "sailfish")]
        let database = database_locking()
            .then(|| {
                self.app_state
                    .pinned()
                    .borrow()
                    .curr_db_ref()
                    .ok()
                    .and_then(|db| db.source().map(|source| source.path.clone()))
            })
            .flatten();

        let handle = actix_rt::task::spawn_blocking(move || -> Result<EncryptedPassword> {
            let stored_pw = stored_pw.ok_or(anyhow!("[Encrypt] No master password stored"))?;
            let encrypted_pw = EncryptedPassword::new(stored_pw)?;
            #[cfg(feature = "sailfish")]
            if let Some(database) = database
                && let Err(err) = encrypted_pw.save(&database)
            {
                warn!("Could not save the master password: {err}");
            }
            Ok(encrypted_pw)
        });

        self.current_operation = Some(handle.abort_handle());

        Box::pin(async move { handle.await? }.into_actor(self).map(
            move |result: Result<EncryptedPassword>, this, _| {
                let binding = this.gui.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(encrypted_pw) => {
                        this.curr_master_pw.replace(Some(encrypted_pw));
                        this.save_authorizer.remember();
                        gui.isMasterPasswordEncrypted = true;
                        gui.masterPasswordStateChanged(true);
                        info!("Master password encrypted.");
                    }
                    Err(err) if this.save_authorizer.method().needs_unlock_key() => {
                        error!("Could not encrypt the master password: {err:?}");
                        gui.errorReceived(format!("{}", err));
                    }
                    Err(err) => warn!("Could not encrypt the master password: {err:?}"),
                }

                if let Some(op) = this.current_operation.as_ref()
                    && op.is_finished()
                {
                    this.current_operation = None;
                }
            },
        ))
    }
}

impl Handler<DecryptMasterPassword> for KeepassRxActor {
    type Result = ResponseActFuture<Self, ()>;
    fn handle(&mut self, msg: DecryptMasterPassword, _: &mut Self::Context) -> Self::Result {
        if msg.short_password.unsecure().is_empty()
            && let Some(confirmation) = self.save_authorizer.confirm(AuthPurpose::Unlock)
        {
            return Box::pin(confirmation.into_actor(self).map(move |result, this, ctx| {
                match result {
                    Ok(()) => {
                        ctx.spawn(this.begin_decrypt(msg.decrypter()));
                    }
                    Err(err) => {
                        warn!("System unlock failed: {err}");
                        this.gui.pinned().borrow().unlockCodeRequired();
                    }
                }
            }));
        }

        self.begin_decrypt(msg.decrypter())
    }
}

impl KeepassRxActor {
    fn begin_decrypt(
        &mut self,
        decrypt: impl FnOnce(EncryptedPassword) -> Result<SecUtf8> + Send + 'static,
    ) -> ResponseActFuture<Self, ()> {
        self.abort_ongoing_operations();

        // Remove from RefCell and keep a backup in case decryption fails.
        let mut maybe_master_pw = self.curr_master_pw.take();
        let backup = maybe_master_pw.clone();
        let master_pw = maybe_master_pw
            .take()
            .ok_or(anyhow!("[Decrypt] No master password stored"));

        let handle = actix_rt::task::spawn_blocking(move || -> Result<SecUtf8> {
            master_pw.and_then(decrypt)
        });

        self.current_operation = Some(handle.abort_handle());

        Box::pin(async move { handle.await? }.into_actor(self).map(
            move |result: Result<SecUtf8>, this, ctx| {
                let binding = this.gui.pinned();
                let mut gui = binding.borrow_mut();

                match result {
                    Ok(secure_decrypted_password) => {
                        this.wrong_codes.set(0);
                        this.stored_master_password
                            .replace(Some(secure_decrypted_password));
                        gui.isMasterPasswordEncrypted = false;
                        gui.masterPasswordStateChanged(false);
                        gui.masterPasswordDecrypted();
                        info!("Master password decrypted.");
                    }
                    Err(err) => {
                        // Put the encrypted password back on failure.
                        this.curr_master_pw.replace(backup);
                        if err.downcast_ref::<IncorrectShortCode>().is_some() {
                            match this.wrong_code_entered(ctx) {
                                WrongCode::Left(left) => gui.unlockAuthRejected(left),
                                WrongCode::LockedOut => gui.codeLimitReached(),
                            }
                        } else {
                            gui.decryptionFailed(QString::from(format!("{}", err)));
                        }
                    }
                }

                if let Some(op) = this.current_operation.as_ref()
                    && op.is_finished()
                {
                    this.current_operation = None;
                }
            },
        ))
    }
}
