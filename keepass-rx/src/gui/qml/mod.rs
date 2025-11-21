use actix::prelude::*;
use actor_macro::observing_model;
use qmeta_async::with_executor;
use qmetaobject::prelude::*;

use crate::actor::{ModelContext, ObservingModelActor};

/// A QObject that is wired to interact with a database entry via the
/// app actor.
#[observing_model]
#[derive(Default, QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiEntry {
    pub(super) base: qt_base_class!(trait QObject),

    pub(super) entryUuid: qt_property!(QString),
    pub(super) currentTotp: qt_property!(QString; NOTIFY currentTotpChanged),
    pub(super) currentTotpValidFor: qt_property!(QString; NOTIFY currentTotpValidForChanged),

    pub(super) currentTotpChanged: qt_signal!(),
    pub(super) currentTotpValidForChanged: qt_signal!(),

    pub(super) updateTotp: qt_method!(fn(&mut self)),
}

#[allow(dead_code, non_snake_case)]
impl RxUiEntry {
    fn init(&mut self, _: ModelContext<Self>) {}

    fn self_actor(&self) -> Option<Addr<ObservingModelActor<Self>>> {
        self._observing_model_registration
            .as_ref()
            .map(|reg| reg.actor.clone())
    }

    #[with_executor]
    pub fn updateTotp(&mut self) {
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();

        let maybe_db = app_state.curr_db();
        let maybe_db = maybe_db.as_deref();

        let totp = maybe_db.and_then(|db| db.get_totp(&self.entryUuid.to_string()).ok());

        if let Some(totp) = totp {
            let totp_code = QString::from(totp.code);
            let valid_for = QString::from(totp.valid_for);

            if totp_code != self.currentTotp {
                self.currentTotp = totp_code;
                self.currentTotpChanged();
            }
            if valid_for != self.currentTotpValidFor {
                self.currentTotpValidFor = valid_for;
                self.currentTotpValidForChanged();
            }
        }
    }
}
