use std::cell::RefCell;
use std::str::FromStr;

use actor_macro::observing_model;
use qmeta_async::with_executor;
use qmetaobject::SimpleListModel;
use qmetaobject::prelude::*;
use uuid::Uuid;

use crate::crypto::MasterKey;
use crate::rx::RxAttachments;
use crate::{app::AppState, rx::virtual_hierarchy::VirtualHierarchyType};

#[derive(Clone, Default, SimpleListItem, Debug)]
#[allow(non_snake_case)]
pub struct RxUiAttachment {
    pub attachmentName: QString,
    pub attachmentSize: i32,
}

fn convert_attachments(value: &RxAttachments, master_key: &MasterKey) -> Vec<RxUiAttachment> {
    value
        .iter()
        .map(|(name, attachment)| RxUiAttachment {
            attachmentName: QString::from(name.as_str()),
            attachmentSize: attachment
                .value_secure(master_key)
                .map(|val| val.len())
                .unwrap_or(0)
                .try_into()
                .ok()
                .unwrap_or(0),
        })
        .collect()
}

/// A QObject that is wired to interact with a database entry via the
/// app actor.
#[observing_model]
#[derive(Default, QObject)]
#[allow(dead_code, non_snake_case)]
pub struct RxUiEntry {
    pub(super) base: qt_base_class!(trait QObject),

    pub(super) entryUuid: qt_property!(QString),

    // TOTP
    pub(super) currentTotp: qt_property!(QString; NOTIFY currentTotpChanged),
    pub(super) currentTotpValidFor: qt_property!(QString; NOTIFY currentTotpValidForChanged),
    pub(super) currentTotpChanged: qt_signal!(),
    pub(super) currentTotpValidForChanged: qt_signal!(),
    pub(super) updateTotp: qt_method!(fn(&mut self)),

    // Attachments
    pub(super) attachments: qt_property!(RefCell<SimpleListModel<RxUiAttachment>>; NOTIFY attachmentsChanged),
    pub(super) attachmentsChanged: qt_signal!(),
    pub(super) loadAttachments: qt_method!(fn(&mut self)),
}

#[allow(dead_code, non_snake_case)]
impl RxUiEntry {
    fn init_from_state(&mut self, _: &AppState) {}
    fn init_from_view(&mut self, _: &VirtualHierarchyType) {}

    #[with_executor]
    pub fn loadAttachments(&mut self) {
        let entry_uuid =
            Uuid::from_str(&self.entryUuid.to_string()).expect("UUID parse failure");
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();
        let maybe_db = app_state.curr_db_ref();

        let attachments = if let Ok(db) = maybe_db {
            let maybe_entry = db.get_entry(entry_uuid);
            let maybe_attach = maybe_entry.as_ref().map(|ent| &ent.attachments);

            maybe_attach
                .map(|att| convert_attachments(att, db.master_key()))
                .unwrap_or_default()
        } else {
            vec![]
        };

        println!("Entries are: {:?}", attachments);

        self.attachments.borrow_mut().reset_data(attachments);
    }

    #[with_executor]
    pub fn updateTotp(&mut self) {
        let app_state = self._app.as_pinned().expect("No app state");
        let app_state = app_state.borrow();

        let maybe_db = app_state.curr_db_ref();

        let totp = maybe_db.and_then(|db| db.get_totp(&self.entryUuid.to_string()));

        if let Ok(totp) = totp {
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
