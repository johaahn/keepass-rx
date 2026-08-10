use gettextrs::{gettext, pgettext};
use qmetaobject::prelude::*;

/// gettext bridge for QML. Exposed as the `Tr` context property so Silica QML
/// can translate through the same gettext/Weblate pipeline the rest of the app
/// uses (the Ubuntu Touch QML uses Lomiri's `i18n.tr`, which is also gettext).
#[derive(QObject, Default)]
#[allow(non_snake_case)]
pub struct RxTranslator {
    base: qt_base_class!(trait QObject),
    tr: qt_method!(fn(&self, msgid: QString) -> QString),
    ctr: qt_method!(fn(&self, context: QString, msgid: QString) -> QString),
}

#[allow(non_snake_case)]
impl RxTranslator {
    pub fn tr(&self, msgid: QString) -> QString {
        QString::from(gettext(msgid.to_string()))
    }

    pub fn ctr(&self, context: QString, msgid: QString) -> QString {
        QString::from(pgettext(context.to_string(), msgid.to_string()))
    }
}
