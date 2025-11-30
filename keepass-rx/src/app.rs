use actix::Addr;
use anyhow::{Result, anyhow};
use libsodium_rs::utils::{SecureVec, vec_utils};
use qmeta_async::with_executor;
use qmetaobject::{QObject, QObjectBox};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::{EncryptedValue, MasterKey};
use crate::gui::actor::KeepassRxActor;
use crate::rx::RxDatabase;
use crate::rx::virtual_hierarchy::VirtualHierarchy;

/// Hackity hack hack hack. Instead of having a running actor system
/// via qmeta_async and using Actix's registry, we'll just keep a
/// public global. Yes very good design.
#[derive(Debug)]
pub struct RxActors {
    pub(crate) gui_actor: Addr<KeepassRxActor>,
}

impl RxActors {
    pub fn set_app_actor(actor: Addr<KeepassRxActor>) {
        ACTORS
            .set(RxActors {
                gui_actor: actor.clone(),
            })
            .expect("Actor addresses already set");
    }

    pub fn app_actor() -> Option<Addr<KeepassRxActor>> {
        ACTORS.get().map(|actors| actors.gui_actor.clone())
    }
}

pub static ACTORS: OnceLock<RxActors> = OnceLock::new();

#[allow(dead_code)]
pub struct KeepassRxApp {
    pub(crate) app_state: Rc<QObjectBox<AppState>>,
}

#[derive(Clone)]
pub enum KeyFile {
    Unencrypted(Vec<u8>),
    Encrypted(EncryptedValue),
}

impl KeyFile {
    pub fn is_unencrypted(&self) -> bool {
        match self {
            KeyFile::Unencrypted(_) => true,
            _ => false,
        }
    }

    pub fn encrypt(self, master_key: &MasterKey) -> Result<KeyFile> {
        match self {
            KeyFile::Encrypted(_) => Ok(self),
            KeyFile::Unencrypted(bytes) => {
                let id = KEY_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
                let mut bytes_buf = vec_utils::secure_vec::<u8>(bytes.len())?;
                bytes_buf.copy_from_slice(bytes.as_slice());
                Ok(KeyFile::Encrypted(EncryptedValue::new(
                    master_key, id, bytes_buf,
                )?))
            }
        }
    }

    pub fn bytes_unencrypted(&self) -> Option<SecureVec<u8>> {
        match self {
            KeyFile::Unencrypted(bytes) => {
                let bytes = bytes.clone();
                let mut bytes_buf = vec_utils::secure_vec::<u8>(bytes.len()).ok()?;
                bytes_buf.copy_from_slice(bytes.as_slice());
                Some(bytes_buf)
            }
            _ => None,
        }
    }

    pub fn bytes(&self, master_key: &MasterKey) -> Result<SecureVec<u8>> {
        let value = match self {
            KeyFile::Unencrypted(bytes) => {
                let bytes = bytes.clone();
                let mut bytes_buf = vec_utils::secure_vec::<u8>(bytes.len())?;
                bytes_buf.copy_from_slice(bytes.as_slice());
                bytes_buf
            }
            KeyFile::Encrypted(value) => value.expose(master_key)?,
        };

        Ok(value)
    }
}

impl Zeroize for KeyFile {
    fn zeroize(&mut self) {
        match self {
            KeyFile::Unencrypted(bytes) => bytes.zeroize(),
            KeyFile::Encrypted(value) => value.zeroize(),
        }
    }
}

impl Default for KeyFile {
    fn default() -> Self {
        KeyFile::Unencrypted(vec![])
    }
}

static KEY_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(QObject, Default)]
#[allow(dead_code)]
pub struct AppState {
    base: qt_base_class!(trait QObject),
    deferred_views: RefCell<Vec<Box<dyn FnOnce(&dyn VirtualHierarchy)>>>,

    current_view: Option<Rc<Box<dyn VirtualHierarchy>>>,
    curr_db: Option<Rc<Zeroizing<RxDatabase>>>,

    master_key: Option<MasterKey>,
    db_key: Option<KeyFile>,
}

impl AppState {
    #[with_executor]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn master_key(&self) -> Option<&MasterKey> {
        self.master_key.as_ref()
    }

    pub fn set_master_key(&mut self, master_key: Option<MasterKey>) {
        self.master_key = master_key;
    }

    pub fn db_key(&self) -> Option<KeyFile> {
        self.db_key.clone()
    }

    pub fn set_db_key(&mut self, key: Option<KeyFile>) {
        self.db_key = key;
    }

    pub fn curr_view(&self) -> Option<Rc<Box<dyn VirtualHierarchy>>> {
        self.current_view.clone()
    }

    pub fn curr_db(&self) -> Result<Rc<Zeroizing<RxDatabase>>> {
        let db = self.curr_db.clone().ok_or(anyhow!("No database set"))?;
        Ok(db)
    }

    pub fn take_db(&mut self) -> Result<Zeroizing<RxDatabase>> {
        let db = self
            .curr_db
            .take()
            .ok_or(anyhow!("Unable to take ownership of database"))?;

        let db = Rc::try_unwrap(db)
            .map_err(|_| anyhow!("Database Rc still has lingering references"))?;
        Ok(db)
    }

    pub fn set_db(&mut self, db: Zeroizing<RxDatabase>) {
        self.curr_db.replace(Rc::new(db));
    }

    pub fn set_curr_view(&mut self, view: Box<dyn VirtualHierarchy>) {
        let view = Rc::new(view);
        for cb in self.deferred_views.take() {
            let view_ref = view.clone();
            // Reason for actix::spawn, see below
            actix::spawn(async move { cb(view_ref.as_ref().as_ref()) });
        }

        self.current_view.replace(view);
    }

    pub fn deferred_with_view(&self, cb: impl FnOnce(&dyn VirtualHierarchy) + 'static) {
        // Calling the callback from within AppState means we have the
        // RefCells that AppState is encapsulated in potentially
        // panic. Spawn the closure on actix, this ensures the borrow
        // of self will have ended.
        if let Some(view) = self.curr_view() {
            actix::spawn(async move { cb(view.as_ref().as_ref()) });
        } else {
            self.deferred_views.borrow_mut().push(Box::new(cb));
        }
    }
}
