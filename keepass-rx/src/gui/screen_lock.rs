use actix::{Addr, Message};

use super::actor::KeepassRxActor;

#[derive(Message)]
#[rtype(result = "()")]
pub struct ScreenLocked;

pub trait ScreenLockWatcher {
    fn start(self: Box<Self>, actor: Addr<KeepassRxActor>);
}

#[cfg(feature = "sailfish")]
pub fn default_watcher() -> Box<dyn ScreenLockWatcher> {
    Box::new(mce::MceWatcher)
}

#[cfg(not(feature = "sailfish"))]
pub fn default_watcher() -> Box<dyn ScreenLockWatcher> {
    Box::new(NoScreenLockWatcher)
}

#[cfg(not(feature = "sailfish"))]
struct NoScreenLockWatcher;

#[cfg(not(feature = "sailfish"))]
impl ScreenLockWatcher for NoScreenLockWatcher {
    fn start(self: Box<Self>, _actor: Addr<KeepassRxActor>) {}
}

#[cfg(any(feature = "sailfish", test))]
const DISPLAY_SIGNAL: &str = "display_status_ind";
#[cfg(any(feature = "sailfish", test))]
const TKLOCK_SIGNAL: &str = "tklock_mode_ind";

#[cfg(any(feature = "sailfish", test))]
fn locks(signal: &str, value: &str) -> bool {
    match signal {
        DISPLAY_SIGNAL => value == "off",
        TKLOCK_SIGNAL => value.starts_with("locked") || value.starts_with("silent-locked"),
        _ => false,
    }
}

#[cfg(feature = "sailfish")]
mod mce {
    use std::thread;

    use actix::Addr;
    use log::warn;
    use zbus::MatchRule;
    use zbus::blocking::{Connection, MessageIterator};
    use zbus::message::Type;

    use super::{DISPLAY_SIGNAL, ScreenLockWatcher, ScreenLocked, TKLOCK_SIGNAL, locks};
    use crate::gui::actor::KeepassRxActor;

    const MCE: &str = "com.nokia.mce";
    const REQUEST_PATH: &str = "/com/nokia/mce/request";
    const REQUEST_INTERFACE: &str = "com.nokia.mce.request";
    const SIGNAL_PATH: &str = "/com/nokia/mce/signal";
    const SIGNAL_INTERFACE: &str = "com.nokia.mce.signal";
    const GET_DISPLAY_STATUS: &str = "get_display_status";
    const GET_TKLOCK_MODE: &str = "get_tklock_mode";
    const KEEPALIVE_START: &str = "req_cpu_keepalive_start";
    const KEEPALIVE_SESSION: &str = "keepassrx-lock";

    struct StateQuery {
        method: &'static str,
        signal: &'static str,
    }

    const INITIAL_STATE: [StateQuery; 2] = [
        StateQuery {
            method: GET_DISPLAY_STATUS,
            signal: DISPLAY_SIGNAL,
        },
        StateQuery {
            method: GET_TKLOCK_MODE,
            signal: TKLOCK_SIGNAL,
        },
    ];

    pub struct MceWatcher;

    impl ScreenLockWatcher for MceWatcher {
        fn start(self: Box<Self>, actor: Addr<KeepassRxActor>) {
            thread::spawn(move || {
                if let Err(err) = watch(&actor) {
                    warn!("Stopped watching the screen lock: {err}");
                }
            });
        }
    }

    fn notify(bus: &Connection, actor: &Addr<KeepassRxActor>, signal: &str, value: &str) {
        if locks(signal, value) {
            if let Err(err) = bus.call_method(
                Some(MCE),
                REQUEST_PATH,
                Some(REQUEST_INTERFACE),
                KEEPALIVE_START,
                &(KEEPALIVE_SESSION,),
            ) {
                warn!("Could not keep the CPU awake while locking: {err}");
            }
            actor.do_send(ScreenLocked);
        }
    }

    fn watch(actor: &Addr<KeepassRxActor>) -> zbus::Result<()> {
        let bus = Connection::system()?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(MCE)?
            .path(SIGNAL_PATH)?
            .interface(SIGNAL_INTERFACE)?
            .build();
        let signals = MessageIterator::for_match_rule(rule, &bus, None)?;

        for StateQuery { method, signal } in INITIAL_STATE {
            let reply = bus.call_method(
                Some(MCE),
                REQUEST_PATH,
                Some(REQUEST_INTERFACE),
                method,
                &(),
            )?;
            let value: String = reply.body().deserialize()?;
            notify(&bus, actor, signal, &value);
        }

        for message in signals {
            let message = message?;
            let header = message.header();
            let Some(member) = header.member() else {
                continue;
            };
            if let Ok(value) = message.body().deserialize::<String>() {
                notify(&bus, actor, member.as_str(), &value);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DISPLAY_SIGNAL, TKLOCK_SIGNAL, locks};

    #[test]
    fn display_off_locks() {
        assert!(locks(DISPLAY_SIGNAL, "off"));
        assert!(!locks(DISPLAY_SIGNAL, "on"));
        assert!(!locks(DISPLAY_SIGNAL, "dim"));
    }

    #[test]
    fn lock_screen_locks() {
        for value in [
            "locked",
            "locked-dim",
            "locked-delay",
            "silent-locked",
            "silent-locked-dim",
        ] {
            assert!(locks(TKLOCK_SIGNAL, value), "{value}");
        }
        assert!(!locks(TKLOCK_SIGNAL, "unlocked"));
        assert!(!locks(TKLOCK_SIGNAL, "silent-unlocked"));
    }

    #[test]
    fn other_signals_do_not_lock() {
        assert!(!locks("charger_state_ind", "off"));
    }
}
