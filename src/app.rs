use actix::Addr;
use anyhow::{Result, anyhow};
use std::sync::{Arc, OnceLock};

use crate::gui::KeepassRx;
use crate::gui::actor::KeepassRxActor;

static APP: OnceLock<KeepassRxApp> = OnceLock::new();

pub fn initialize(app: KeepassRxApp) -> Result<()> {
    APP.set(app)
        .map_err(|_| anyhow!("App already initialized!"))
}

pub fn current() -> &'static KeepassRxApp {
    APP.get().expect("App not initialized.")
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct KeepassRxApp {
    app_actor: Addr<KeepassRxActor>,
}

impl KeepassRxApp {
    pub fn new(app_actor: Addr<KeepassRxActor>) -> Self {
        Self {
            app_actor: app_actor,
        }
    }

    pub fn app_actor(&self) -> Addr<KeepassRxActor> {
        self.app_actor.clone()
    }
}
