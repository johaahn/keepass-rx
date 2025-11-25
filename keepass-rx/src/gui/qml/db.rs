use actix::prelude::*;
use actor_macro::observing_model;
use qmeta_async::with_executor;
use qmetaobject::prelude::*;

use crate::{
    actor::{ActorConnected, ConnectedModelActor, ModelContext},
    gui::RxDbType,
    rx::virtual_hierarchy::VirtualHierarchy,
};

#[derive(Message)]
#[rtype(result = "()")]
struct OpenDatabase;

#[observing_model]
#[derive(Default, QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiDatabase {
    pub(super) base: qt_base_class!(trait QObject),
    pub(super) databaseName: qt_property!(QString),
    pub(super) databaseType: qt_property!(RxDbType),

    pub(super) open: qt_method!(fn(&self)),
}

impl ActorConnected<OpenDatabase> for RxUiDatabase {
    type Context = ModelContext<Self>;

    fn app_state(&self) -> &crate::app::AppState {
        self._app.as_ref().expect("No app state available")
    }

    fn handle(&mut self, ctx: Self::Context, message: OpenDatabase)
    where
        Self: Sized + QObject,
    {
        let system = System::try_current();
        //let jank = System::

        println!("Hello from connected actor. The system is: {:?}", system);
    }
}

impl RxUiDatabase {
    fn init_from_view(&mut self, _: &dyn VirtualHierarchy) {}

    fn connected_actor(&self) -> Option<Addr<ConnectedModelActor<Self>>> {
        self._connected_model_registration
            .as_ref()
            .map(|reg| reg.actor.clone())
    }

    #[with_executor]
    fn open(&self) {
        // Communicate with global object here. Stopgap.
        if let Some(actor) = self.connected_actor() {
            actix::spawn(actor.send(OpenDatabase));
        } else {
            println!("No actor connection active?");
        }
    }
}
