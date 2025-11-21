use actix::Addr;
use anyhow::{Result, anyhow};
use qmeta_async::with_executor;
use qmetaobject::{QObject, QObjectBox, QPointer};
use std::cell::{Ref, RefCell};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use zeroize::Zeroizing;

use crate::gui::KeepassRx;
use crate::gui::actor::KeepassRxActor;
use crate::rx::RxDatabase;
use crate::rx::virtual_hierarchy::VirtualHierarchy;

pub struct KeepassRxApp {
    pub(crate) app_state: Arc<QObjectBox<AppState>>,
    pub(crate) gui_actor: Addr<KeepassRxActor>,
}

#[derive(QObject, Default)]
#[allow(dead_code)]
pub struct AppState {
    base: qt_base_class!(trait QObject),
    current_view: Option<Rc<Box<dyn VirtualHierarchy>>>,
    curr_db: Option<Rc<Zeroizing<RxDatabase>>>,
}

impl AppState {
    #[with_executor]
    pub fn new() -> Self {
        Self {
            //app_actor: Some(app_actor),
            ..Default::default()
        }
    }

    pub fn curr_view(&self) -> Result<Rc<Box<dyn VirtualHierarchy>>> {
        let view = self
            .current_view
            .clone()
            .ok_or(anyhow!("No current view"))?;

        Ok(view)
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
        self.current_view.replace(Rc::new(view));
    }
}
