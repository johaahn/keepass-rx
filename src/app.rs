use actix::Addr;
use qmetaobject::{QObject, QObjectBox};
use std::sync::Arc;

use crate::gui::{KeepassRx, KeepassRxActor};

#[allow(dead_code)]
pub struct KeepassRxApp {
    pub gui: Arc<QObjectBox<KeepassRx>>,
    pub gui_actor: Addr<KeepassRxActor>,
}
