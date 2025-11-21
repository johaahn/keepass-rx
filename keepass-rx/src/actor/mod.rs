use std::rc::Rc;

use actix::prelude::*;
use qmetaobject::QObject;
use qmetaobject::QPointer;

pub use actor_macro::observing_model;

use crate::app::AppState;

// TODO temporary
#[derive(Message)]
#[rtype(result = "()")]
pub struct ActixEvent {
    pub event_name: String,
}

pub trait EventObserving<M>
where
    M: actix::Message,
{
    type Context;

    fn app_state(&self) -> &AppState;

    fn observe(&mut self, ctx: Self::Context, message: M)
    where
        Self: Sized + QObject;
}

pub struct ModelContext<T: QObject + 'static> {
    pub(crate) addr: Addr<ObservingModelActor<T>>,
}

impl<T: QObject + 'static> ModelContext<T> {
    pub fn addr(&self) -> Addr<ObservingModelActor<T>> {
        self.addr.clone()
    }
}

/// An actor that accompanies the `ObservingModel`, responsible to
/// dispatch events to the contained model. The contained model is a
/// weak pointer, such that the actor will stop when the model goes
/// out of scope.
pub struct ObservingModelActor<T: QObject> {
    pub(super) model: QPointer<T>,
}

impl<T: QObject + 'static> actix::Actor for ObservingModelActor<T> {
    type Context = actix::Context<Self>;
}

impl<M, T: QObject + 'static> actix::Handler<M> for ObservingModelActor<T>
where
    T: EventObserving<M, Context = ModelContext<T>>,
    M: Sized + actix::Message<Result = ()>,
{
    type Result = ();

    fn handle(&mut self, event: M, ctx: &mut Self::Context) -> Self::Result {
        match self.model.as_pinned() {
            Some(model) => {
                let mut model = model.borrow_mut();
                let ctx = ModelContext {
                    addr: ctx.address(),
                };
                model.observe(ctx, event);
            }
            None => {
                // In principle, the actor should have gotten stopped
                // when the model got dropped, because the actor's
                // only strong reference is contained in the
                // ObservingModel.
                println!("Model got dropped, stopping actor execution.");
                // XXX What is the difference between stop and terminate?
                ctx.stop();
            }
        }
    }
}

pub struct ObservingModelRegistration<T: QObject + 'static> {
    pub(crate) actor: actix::Addr<ObservingModelActor<T>>,
}
