use actix::prelude::*;
use anyhow::{Result, anyhow};
use colors::wash_out_by_blending;
use models::{RxPageType, RxUiContainer};
use qmeta_async::with_executor;
use qmetaobject::*;
use secstr::SecUtf8;
use std::fs::{create_dir_all, remove_dir_all};
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

pub(crate) mod actor;
pub(crate) mod colors;
pub(crate) mod icons;
pub(crate) mod models;
pub(crate) mod utils;

use actor::*;
use utils::*;

#[derive(Default, QEnum, Clone, Copy)]
#[repr(C)]
pub enum RxGuiState {
    #[default]
    NotOpen,
    Open,
    Locked,
}

fn gui_state_from_string(qval: &QString) -> RxGuiState {
    match qval.to_string().as_str() {
        "NotOpen" => RxGuiState::NotOpen,
        "Open" => RxGuiState::Open,
        "Locked" => RxGuiState::Locked,
        _ => panic!("Invalid GUI state: {}", qval),
    }
}

fn gui_state_to_string(gui_state: &RxGuiState) -> QString {
    match gui_state {
        RxGuiState::NotOpen => "NotOpen",
        RxGuiState::Open => "Open",
        RxGuiState::Locked => "Locked",
    }
    .into()
}

impl QMetaType for RxGuiState {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(gui_state_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(gui_state_to_string);
}

#[derive(Default, QEnum, Clone, Copy)]
#[repr(C)]
pub enum RxViewMode {
    #[default]
    All,
    Templates,
}

fn view_mode_from_string(qval: &QString) -> RxViewMode {
    match qval.to_string().as_str() {
        "All" => RxViewMode::All,
        "Templates" => RxViewMode::Templates,
        _ => panic!("Invalid view mode: {}", qval),
    }
}

fn view_mode_to_string(view_mode: &RxViewMode) -> QString {
    match view_mode {
        RxViewMode::All => "All",
        RxViewMode::Templates => "Templates",
    }
    .into()
}

impl QMetaType for RxViewMode {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(view_mode_from_string);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(view_mode_to_string);
}

#[derive(QObject, Default)]
#[allow(non_snake_case, dead_code)]
pub struct KeepassRx {
    base: qt_base_class!(trait QObject),
    actor: Option<Addr<KeepassRxActor>>,
    last_db: Option<String>,
    container_stack: Vec<RxUiContainer>,

    guiState: qt_property!(RxGuiState),
    viewMode: qt_property!(RxViewMode; READ getViewMode WRITE setViewMode NOTIFY viewModeChanged),
    lastDB: qt_property!(QString; READ getLastDB WRITE setLastDB NOTIFY lastDbChanged),
    databaseOpen: qt_property!(bool),
    isMasterPasswordEncrypted: qt_property!(bool; NOTIFY masterPasswordStateChanged),
    rootGroupUuid: qt_property!(QString; NOTIFY rootGroupUuidChanged),

    // page management
    currentContainer: qt_property!(QVariant; NOTIFY currentContainerChanged),
    pushContainer: qt_method!(fn(&self, container_uuid: QString)),
    popContainer: qt_method!(fn(&self)),

    // database management
    listImportedDatabases: qt_method!(fn(&self)),
    importDatabase: qt_method!(fn(&self, path: String)),
    getMetadata: qt_method!(fn(&self)),
    openDatabase: qt_method!(fn(&mut self, db_name: String, key_path: QString)),
    closeDatabase: qt_method!(fn(&mut self)),
    deleteDatabase: qt_method!(fn(&self, db_name: String)),

    // group and entry management
    getRootGroup: qt_method!(fn(&self)),
    getTemplates: qt_method!(fn(&self)),
    getTemplate: qt_method!(fn(&self, template_uuid: QString)),
    getGroup: qt_method!(fn(&self, group_uuid: QString)),
    getRootEntries: qt_method!(fn(&self, search_term: QString)),
    getEntries: qt_method!(fn(&self, group_uuid: QString, search_term: QString)),
    getSingleEntry: qt_method!(fn(&self, entry_uuid: QString)),
    getTotp: qt_method!(fn(&self, entry_uuid: QString)),
    getFieldValue: qt_method!(fn(&self, entry_uuid: QString, field_name: QString)),

    // easy-open management
    storeMasterPassword: qt_method!(fn(&self, master_password: QString)),
    encryptMasterPassword: qt_method!(fn(&self)),
    decryptMasterPassword: qt_method!(fn(&self, short_password: QString)),
    invalidateMasterPassword: qt_method!(fn(&self)),
    checkLockingStatus: qt_method!(fn(&self)),

    // misc utility functions
    washOutColor: qt_method!(fn(&self, hex_color: QString) -> QVariantMap),

    // page signals
    currentContainerChanged: qt_signal!(new_container: QVariant),

    // db management signals
    lastDbChanged: qt_signal!(value: QString),
    viewModeChanged: qt_signal!(value: RxViewMode),
    fileListingCompleted: qt_signal!(),
    rootGroupUuidChanged: qt_signal!(),
    metadataReceived: qt_signal!(metadata: QVariantMap),
    databaseImported: qt_signal!(db_name: QString),
    databaseOpened: qt_signal!(),
    databaseClosed: qt_signal!(),
    databaseDeleted: qt_signal!(db_name: QString),
    databaseOpenFailed: qt_signal!(message: String),

    // data signals
    templatesReceived: qt_signal!(templates: QVariantList),
    groupReceived: qt_signal!(parent_group_uuid: QString, this_group_uuid: QString, this_group_name: QString),
    templateReceived: qt_signal!(this_template_uuid: QString, this_template_name: QString),
    entriesReceived: qt_signal!(entries: QVariantList),
    errorReceived: qt_signal!(error: String),
    totpReceived: qt_signal!(totp: QVariantMap),
    singleEntryReceived: qt_signal!(entry: QVariant),
    fieldValueReceived: qt_signal!(entry_uuid: QString, field_name: QString, field_value: QString),

    // easy-open signals
    masterPasswordStored: qt_signal!(),
    masterPasswordInvalidated: qt_signal!(),
    masterPasswordStateChanged: qt_signal!(encrypted: bool),
    masterPasswordDecrypted: qt_signal!(),
    lockingStatusReceived: qt_signal!(status: String),
    decryptionFailed: qt_signal!(error: QString),
}

#[allow(non_snake_case)]
impl KeepassRx {
    pub fn new(last_db: Option<String>) -> Self {
        KeepassRx {
            last_db: last_db,
            ..Default::default()
        }
    }

    pub fn getViewMode(&self) -> RxViewMode {
        self.viewMode
    }

    pub fn setViewMode(&mut self, mode: RxViewMode) {
        self.viewMode = mode;

        self.container_stack.clear();

        if let RxViewMode::All = mode {
            self.container_stack.push(RxUiContainer {
                uuid: Uuid::from_str(&self.rootGroupUuid.to_string()).unwrap(),
                page_type: RxPageType::Group,
                is_root: true,
            });
        } else {
            self.container_stack.push(RxUiContainer {
                uuid: Uuid::default(),
                page_type: RxPageType::Template,
                is_root: true,
            });
        }

        let container = QVariantMap::from(&self.container_stack[0]);
        self.viewModeChanged(mode);
        self.currentContainerChanged(container.into());
    }

    pub fn getLastDB(&self) -> QString {
        self.last_db.clone().map(QString::from).unwrap_or_default()
    }

    pub fn setLastDB(&mut self, last_db: QString) {
        let new_last_db = if !last_db.is_null() {
            Some(last_db.to_string())
        } else {
            None
        };

        let change = self.last_db != new_last_db;

        if change {
            let last_db_file = app_data_path().join("last-db");
            let result = match new_last_db {
                Some(ref db) => std::fs::write(last_db_file, db),
                None => std::fs::write(last_db_file, "".to_string()),
            };

            if let Err(err) = result {
                println!("Unable to write last-db file: {}", err);
            }

            self.last_db = new_last_db.clone();
            self.lastDbChanged(new_last_db.map(QString::from).unwrap_or_default());
        }
    }

    #[with_executor]
    pub fn listImportedDatabases(&self) {
        let list_dbs = || -> Result<()> {
            let dbs = std::fs::read_dir(imported_databases_path())?;

            for db in dbs {
                self.databaseImported(QString::from(
                    db?.file_name().to_string_lossy().to_string(),
                ));
            }

            Ok(())
        };

        match list_dbs() {
            Ok(_) => self.fileListingCompleted(),
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    /// Copy a chosen database file into the local data structure. This
    /// is completely sync because we need to finalize() the transfer
    /// in QML from the same scope as this method call.
    #[with_executor]
    pub fn importDatabase(&self, path: String) {
        let copy_file = move || -> Result<String> {
            let source = Path::new(&path);
            let db_name = source
                .file_name()
                .ok_or(anyhow!("No filename found"))?
                .to_string_lossy()
                .into_owned();

            let dest_dir = imported_databases_path();
            let dest = dest_dir.join(&db_name);

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
            Ok(db_name)
        };

        match copy_file() {
            Ok(db_name) => self.databaseImported(QString::from(db_name)),
            Err(err) => self.databaseOpenFailed(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn getMetadata(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetMetadata));
    }

    #[with_executor]
    pub fn openDatabase(&mut self, db_name: String, key_path: QString) {
        let key_path = match key_path {
            kp if !kp.is_null() && !kp.is_empty() => Some(kp.to_string()),
            _ => None,
        };

        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(OpenDatabase { db_name, key_path }));
    }

    #[with_executor]
    pub fn closeDatabase(&mut self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(CloseDatabase));
    }

    #[with_executor]
    pub fn deleteDatabase(&self, db_name: String) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(DeleteDatabase { db_name }));
    }

    #[with_executor]
    pub fn getRootGroup(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetGroup::root()));
    }

    #[with_executor]
    pub fn getGroup(&self, group_uuid: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        let maybe_uuid = Uuid::from_str(&group_uuid.to_string());

        match maybe_uuid {
            Ok(group_uuid) => {
                actix::spawn(actor.send(GetGroup::for_uuid(group_uuid)));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn getTemplate(&self, template_uuid: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        let maybe_uuid = Uuid::from_str(&template_uuid.to_string());

        match maybe_uuid {
            Ok(group_uuid) => {
                actix::spawn(actor.send(GetTemplate::for_uuid(group_uuid)));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn getRootEntries(&self, search_term: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        let search_term = match search_term {
            term if !term.is_null() => Some(term.to_string()),
            _ => None,
        };

        actix::spawn(actor.send(GetEntries::root(search_term)));
    }

    #[with_executor]
    pub fn getEntries(&self, group_uuid: QString, search_term: QString) {
        let maybe_uuid = Uuid::from_str(&group_uuid.to_string());
        let search_term = match search_term {
            term if !term.trimmed().is_empty() && !term.is_null() => Some(term.to_string()),
            _ => None,
        };

        let actor = self.actor.clone().expect("Actor not initialized");

        match maybe_uuid {
            Ok(group_uuid) => {
                actix::spawn(actor.send(GetEntries::for_uuid(group_uuid, search_term)));
            }
            Err(err) => self.errorReceived(format!("GetEntries: error parsing UUID: {}", err)),
        }
    }

    #[with_executor]
    pub fn getSingleEntry(&self, entry_uuid: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        let maybe_uuid = Uuid::from_str(&entry_uuid.to_string());

        match maybe_uuid {
            Ok(entry_uuid) => {
                actix::spawn(actor.send(GetSingleEntry { entry_uuid }));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
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

    #[with_executor]
    pub fn getTemplates(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetTemplates));
    }

    #[with_executor]
    pub fn getFieldValue(&self, entry_uuid: QString, field_name: QString) {
        let maybe_uuid = Uuid::from_str(&entry_uuid.to_string());
        let actor = self.actor.clone().expect("Actor not initialized");

        match maybe_uuid {
            Ok(entry_uuid) => {
                actix::spawn(actor.send(GetFieldValue {
                    entry_uuid,
                    field_name: field_name.into(),
                }));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn pushContainer(&self, container_uuid: QString) {
        let maybe_uuid = Uuid::from_str(&container_uuid.to_string());
        let actor = self.actor.clone().expect("Actor not initialized");
        match maybe_uuid {
            Ok(container_id) => {
                actix::spawn(actor.send(PushContainer(container_id)));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn popContainer(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(PopContainer));
    }

    #[with_executor]
    pub fn washOutColor(&self, hex_color: QString) -> QVariantMap {
        wash_out_by_blending(&hex_color.to_string(), 0.5)
            .expect("No color?!")
            .into()
    }
}
