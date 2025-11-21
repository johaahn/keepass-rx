use actix::Addr;
use anyhow::{Result, anyhow};
use qmeta_async::with_executor;
use qmetaobject::{QObject, QObjectBox, QPointer};
use std::cell::{Ref, RefCell};
use std::sync::{Arc, OnceLock};
use zeroize::Zeroizing;

use crate::gui::KeepassRx;
use crate::gui::actor::KeepassRxActor;
use crate::rx::RxDatabase;

pub struct KeepassRxApp {
    pub(crate) app_state: Arc<QObjectBox<AppState>>,
    pub(crate) gui_actor: Addr<KeepassRxActor>,
}

#[derive(QObject, Default)]
#[allow(dead_code)]
pub struct AppState {
    base: qt_base_class!(trait QObject),
    //app_actor: Option<Addr<KeepassRxActor>>,
    curr_db: RefCell<Option<Zeroizing<RxDatabase>>>,
}

impl AppState {
    #[with_executor]
    pub fn new() -> Self {
        Self {
            //app_actor: Some(app_actor),
            ..Default::default()
        }
    }

    pub fn curr_db(&self) -> Ref<'_, Option<Zeroizing<RxDatabase>>> {
        self.curr_db.borrow()
    }

    pub fn take_db(&self) -> Option<Zeroizing<RxDatabase>> {
        self.curr_db.take()
    }

    pub fn set_db(&self, db: Option<Zeroizing<RxDatabase>>) {
        self.curr_db.replace(db);
    }
}
