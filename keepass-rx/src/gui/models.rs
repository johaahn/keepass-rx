use qmetaobject::{QString, QVariantMap};
use uuid::Uuid;

/// What group/template container we are in. Used in conjunction with
/// RxViewMode to determine if we should be able to travel back up the
/// tree and so on.
#[derive(Default, Clone)]
pub struct RxUiContainer {
    pub uuid: Uuid,
    pub is_root: bool,
    pub instructions: Option<String>,
}

impl From<&RxUiContainer> for QVariantMap {
    fn from(value: &RxUiContainer) -> Self {
        let mut qvar = QVariantMap::default();
        qvar.insert(
            "containerUuid".into(),
            QString::from(value.uuid.to_string()).into(),
        );

        qvar.insert("isRoot".into(), value.is_root.into());

        qvar.insert(
            "instructions".into(),
            value
                .instructions
                .as_deref()
                .map(QString::from)
                .unwrap_or_default()
                .into(),
        );

        qvar
    }
}
