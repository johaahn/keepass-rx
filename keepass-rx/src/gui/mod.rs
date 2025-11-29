use actix::prelude::*;
use anyhow::{Result, anyhow};
use colors::wash_out_by_blending;
use gettextrs::pgettext;
use qmeta_async::with_executor;
use qmetaobject::*;
use secstr::SecUtf8;
use std::fs::{create_dir_all, remove_dir_all};
use std::path::Path;
use std::str::FromStr;
use unicase::UniCase;
use uuid::Uuid;

pub(crate) mod actor;
pub(crate) mod colors;
pub(crate) mod instructions;
pub(crate) mod qml;
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

#[derive(Debug, Default, QEnum, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
#[repr(C)]
pub enum RxDbType {
    #[default]
    Imported = 0,
    Synced = 1,
}

impl RxDbType {
    fn translate(&self) -> String {
        match self {
            RxDbType::Imported => pgettext("A database imported via ContentHub", "Imported"),
            RxDbType::Synced => pgettext("A database managed by external file sync", "Synced"),
        }
    }
}

impl std::fmt::Display for RxDbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RxDbType::Imported => write!(f, "Imported"),
            RxDbType::Synced => write!(f, "Synced"),
        }
    }
}

impl TryFrom<String> for RxDbType {
    type Error = anyhow::Error;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        RxDbType::from_str(&value)
    }
}

impl TryFrom<&String> for RxDbType {
    type Error = anyhow::Error;
    fn try_from(value: &String) -> std::result::Result<Self, Self::Error> {
        RxDbType::from_str(value)
    }
}

impl FromStr for RxDbType {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "Imported" => Ok(RxDbType::Imported),
            "Synced" => Ok(RxDbType::Synced),
            _ => Err(anyhow!("Invalid RxDbType: {}", value)),
        }
    }
}

impl QMetaType for RxDbType {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> =
        Some(|qval: &QString| RxDbType::try_from(qval.to_string()).unwrap());

    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> =
        Some(|db_type: &Self| match db_type {
            RxDbType::Imported => "Imported".into(),
            RxDbType::Synced => "Synced".into(),
        });

    fn from_qvariant(variant: QVariant) -> Option<Self> {
        if variant.is_null() {
            return None;
        }

        // Probably should check this here.
        match variant.to_int() {
            0 => Some(RxDbType::Imported),
            1 => Some(RxDbType::Synced),
            _ => None,
        }
    }

    fn to_qvariant(&self) -> QVariant {
        QVariant::from(*self as u32)
    }
}

#[derive(Debug, Default, QEnum, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum RxViewMode {
    #[default]
    All,
    Templates,
    Totp,
    Tags,
}

fn view_mode_from_string(qval: &QString) -> RxViewMode {
    match qval.to_string().as_str() {
        "All" => RxViewMode::All,
        "Templates" => RxViewMode::Templates,
        "Totp" => RxViewMode::Totp,
        "Tags" => RxViewMode::Tags,
        _ => panic!("Invalid view mode: {}", qval),
    }
}

fn view_mode_to_string(view_mode: &RxViewMode) -> QString {
    match view_mode {
        RxViewMode::All => "All",
        RxViewMode::Templates => "Templates",
        RxViewMode::Totp => "Totp",
        RxViewMode::Tags => "Tags",
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

    guiState: qt_property!(RxGuiState),
    viewMode: qt_property!(RxViewMode; READ getViewMode WRITE setViewMode NOTIFY viewModeChanged),
    databaseOpen: qt_property!(bool),
    isMasterPasswordEncrypted: qt_property!(bool; NOTIFY masterPasswordStateChanged),
    rootGroupUuid: qt_property!(QString; NOTIFY rootGroupUuidChanged),
    metadata: qt_property!(QVariant; NOTIFY metadataChanged),

    // database management
    listImportedDatabases: qt_method!(fn(&self)),
    importDatabase: qt_method!(fn(&self, path: String)),
    getMetadata: qt_method!(fn(&self)),
    closeDatabase: qt_method!(fn(&mut self)),
    deleteDatabase: qt_method!(fn(&self, db_name: String)),

    // group and entry management
    getRootContainer: qt_method!(fn(&self)),
    getContainer: qt_method!(fn(&self, container_uuid: QString)),
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

    // db management signals
    viewModeChanged: qt_signal!(value: RxViewMode),
    fileListingCompleted: qt_signal!(),
    rootGroupUuidChanged: qt_signal!(),
    metadataReceived: qt_signal!(metadata: QVariantMap),
    databaseImported: qt_signal!(db_name: QString, db_type: RxDbType),
    databaseOpened: qt_signal!(),
    databaseClosed: qt_signal!(),
    databaseDeleted: qt_signal!(db_name: QString),
    databaseOpenFailed: qt_signal!(message: String),
    keyFileSet: qt_signal!(),

    // data signals
    metadataChanged: qt_signal!(),
    containerReceived: qt_signal!(this_container_uuid: QString, this_container_name: QString),
    entriesReceived: qt_signal!(entries: QStringList),
    errorReceived: qt_signal!(error: String),
    totpReceived: qt_signal!(totp: QVariantMap),
    singleEntryReceived: qt_signal!(entry: QVariant),
    fieldValueReceived: qt_signal!(entry_uuid: QString, field_name: QString, field_value: QString, field_extra: QString),

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
    pub fn new() -> Self {
        KeepassRx {
            ..Default::default()
        }
    }

    pub fn getViewMode(&self) -> RxViewMode {
        self.viewMode
    }

    #[with_executor]
    pub fn setViewMode(&mut self, mode: RxViewMode) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(SetViewMode(mode)));
    }

    #[with_executor]
    pub fn listImportedDatabases(&self) {
        let want_file = |db: &std::fs::DirEntry| -> bool {
            match db.file_name().into_string() {
                Ok(file_str) => file_str.ends_with(".kdbx") || file_str.ends_with(".kdb"),
                Err(os_str) => {
                    let file_str = os_str.to_string_lossy();
                    file_str.ends_with(".kdbx") || file_str.ends_with(".kdb")
                }
            }
        };

        let list_dbs = || -> Result<()> {
            create_dir_all(&imported_databases_path())?;
            create_dir_all(&synced_databases_path())?;

            let imported_dbs = std::fs::read_dir(imported_databases_path())?;
            let synced_dbs = std::fs::read_dir(synced_databases_path())?;
            let mut dbs = vec![];

            struct DbListing(std::fs::DirEntry, RxDbType);

            for db in imported_dbs {
                let db = db?;
                if want_file(&db) {
                    dbs.push(DbListing(db, RxDbType::Imported));
                }
            }

            for db in synced_dbs {
                let db = db?;
                if want_file(&db) {
                    dbs.push(DbListing(db, RxDbType::Synced));
                }
            }

            dbs.sort_by(|DbListing(this, _), DbListing(that, _)| {
                let name1 = UniCase::new(this.file_name().to_string_lossy()).to_folded_case();
                let name2 = UniCase::new(that.file_name().to_string_lossy()).to_folded_case();
                name1.cmp(&name2)
            });

            for DbListing(db, db_type) in dbs {
                self.databaseImported(
                    QString::from(db.file_name().to_string_lossy().to_string()),
                    db_type,
                );
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
            Ok(db_name) => self.databaseImported(QString::from(db_name), RxDbType::Imported),
            Err(err) => self.databaseOpenFailed(format!("{}", err)),
        }
    }

    #[with_executor]
    pub fn getMetadata(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetMetadata));
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
    pub fn getRootContainer(&self) {
        let actor = self.actor.clone().expect("Actor not initialized");
        actix::spawn(actor.send(GetContainer::root()));
    }

    #[with_executor]
    pub fn getContainer(&self, group_uuid: QString) {
        let actor = self.actor.clone().expect("Actor not initialized");
        let maybe_uuid = Uuid::from_str(&group_uuid.to_string());

        match maybe_uuid {
            Ok(group_uuid) => {
                actix::spawn(actor.send(GetContainer::for_uuid(group_uuid)));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
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
    pub fn washOutColor(&self, hex_color: QString) -> QVariantMap {
        wash_out_by_blending(&hex_color.to_string(), 0.5)
            .expect("No color generated")
            .into()
    }
}
