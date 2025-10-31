use actix::Addr;
use qmetaobject::QObjectBox;
use std::sync::Arc;

use crate::gui::KeepassRx;
use crate::gui::actor::KeepassRxActor;

#[allow(dead_code)]
pub struct KeepassRxApp {
    pub gui: Arc<QObjectBox<KeepassRx>>,
    pub gui_actor: Addr<KeepassRxActor>,
}
