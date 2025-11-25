/*
 * Copyright (C) 2019-2025 Ruben De Smet, Markus Törnqvist, 2025 projectmoon
 *
 * Modified Whisperfish actor binding code via proc-macro. The license of
 * Whisperfish follows:
 *
 * Whisperfish is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or (at
 * your option) any later version.
 * Whisperfish is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
 * General Public License for more details.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see
 * <https://www.gnu.org/licenses/>.
 */
use actix::SystemRegistry;
use actix::prelude::*;
use qmeta_async::with_executor;
use qmetaobject::QObject;
use qmetaobject::QPointer;

use crate::app::AppState;
use crate::gui::actor::KeepassRxActor;

#[allow(dead_code)]
pub trait ActorConnected<M>
where
    M: actix::Message,
{
    type Context;

    fn app_state(&self) -> &AppState;

    fn handle(&mut self, ctx: Self::Context, message: M)
    where
        Self: Sized + QObject;
}

pub struct ModelContext<T: QObject + 'static> {
    pub(crate) addr: Addr<ConnectedModelActor<T>>,
}

#[allow(dead_code)]
impl<T: QObject + 'static> ModelContext<T> {
    pub fn addr(&self) -> Addr<ConnectedModelActor<T>> {
        self.addr.clone()
    }
}

/// An actor that accompanies the `ObservingModel`, responsible to
/// dispatch events to the contained model. The contained model is a
/// weak pointer, such that the actor will stop when the model goes
/// out of scope.
pub struct ConnectedModelActor<T: QObject> {
    pub(super) model: QPointer<T>,
}

impl<T: QObject + 'static> actix::Actor for ConnectedModelActor<T> {
    type Context = actix::Context<Self>;
}

impl<M, T: QObject + 'static> actix::Handler<M> for ConnectedModelActor<T>
where
    T: ActorConnected<M, Context = ModelContext<T>>,
    M: Sized + actix::Message<Result = ()>,
{
    type Result = ();

    #[with_executor]
    fn handle(&mut self, event: M, ctx: &mut Self::Context) -> Self::Result {
        match self.model.as_pinned() {
            Some(model) => {
                println!("from registry shit");
                let stuff = KeepassRxActor::from_registry();
                println!("stuff is: {:?}", stuff);
                let jank = System::try_current();
                println!("jank is: {:?}", jank);
                let mut model = model.borrow_mut();
                let ctx = ModelContext {
                    addr: ctx.address(),
                };
                model.handle(ctx, event);
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

pub struct ConnectedModelRegistration<T: QObject + 'static> {
    pub(crate) actor: actix::Addr<ConnectedModelActor<T>>,
}
