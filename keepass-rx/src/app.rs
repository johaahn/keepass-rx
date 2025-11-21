use actix::Addr;
use anyhow::{Result, anyhow};
use qmeta_async::with_executor;
use qmetaobject::{QObject, QObjectBox};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use zeroize::Zeroizing;

use crate::gui::actor::KeepassRxActor;
use crate::rx::RxDatabase;
use crate::rx::virtual_hierarchy::VirtualHierarchy;

#[allow(dead_code)]
pub struct KeepassRxApp {
    pub(crate) app_state: Arc<QObjectBox<AppState>>,
    pub(crate) gui_actor: Addr<KeepassRxActor>,
}

#[derive(QObject, Default)]
#[allow(dead_code)]
pub struct AppState {
    base: qt_base_class!(trait QObject),

    deferred_views: RefCell<Vec<Box<dyn FnOnce(&dyn VirtualHierarchy)>>>,

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
        // Calling the callback from within AppState means we have the RefCells that AppState is
        // encapsulated in potentially panic.
        // Spawn the closure on actix, this ensures the borrow of self will have ended.
        if let Some(view) = self.curr_view() {
            actix::spawn(async move { cb(view.as_ref().as_ref()) });
        } else {
            self.deferred_views.borrow_mut().push(Box::new(cb));
        }
    }
}
