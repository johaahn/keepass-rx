use std::str::FromStr;

use cpp::cpp;
use qmeta_async::with_executor;
use qmetaobject::QString;
use uuid::Uuid;

use super::KeepassRx;
use super::actor::GetFieldValue;

cpp! {{
    #include <QtGui/QClipboard>
    #include <QtGui/QGuiApplication>
}}

pub fn set_clipboard_text(text: &QString) {
    unsafe {
        cpp!([text as "const QString *"] {
            QGuiApplication::clipboard()->setText(*text);
        })
    }
}

pub fn clear_clipboard() {
    unsafe {
        cpp!([] {
            QGuiApplication::clipboard()->clear();
        })
    }
}

#[allow(non_snake_case)]
impl KeepassRx {
    #[with_executor]
    pub fn getFieldValue(&self, entry_uuid: QString, field_name: QString) {
        self.request_field_value(entry_uuid, field_name, "copy");
    }

    #[with_executor]
    pub fn revealFieldValue(&self, entry_uuid: QString, field_name: QString) {
        self.request_field_value(entry_uuid, field_name, "reveal");
    }

    #[with_executor]
    pub fn copyFieldValue(&self, entry_uuid: QString, field_name: QString) {
        self.request_field_value(entry_uuid, field_name, "clipboard");
    }

    fn request_field_value(&self, entry_uuid: QString, field_name: QString, purpose: &str) {
        let maybe_uuid = Uuid::from_str(&entry_uuid.to_string());
        let actor = self.actor_ref();

        match maybe_uuid {
            Ok(entry_uuid) => {
                actix::spawn(actor.send(GetFieldValue {
                    entry_uuid,
                    field_name: field_name.into(),
                    purpose: purpose.to_string(),
                }));
            }
            Err(err) => self.errorReceived(format!("{}", err)),
        }
    }

    pub fn clearClipboard(&self) {
        clear_clipboard();
    }
}
