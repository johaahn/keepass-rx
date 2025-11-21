use paste::paste;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use actix::{Actor, Addr};
use actor_macro::observing_model;
use anyhow::anyhow;
use gettextrs::npgettext;
use qmeta_async::with_executor;
use qmetaobject::{
    QMetaType, QObject, QObjectPinned, QObjectRefMut, QPointer, QString, QStringList,
    QVariant, QVariantList, QVariantMap,
};
use uuid::Uuid;

use crate::actor::{ActixEvent, EventObserving, ModelContext, ObservingModelActor};
use crate::app::{self, AppState};
use crate::rx::virtual_hierarchy::{RxViewFeature, VirtualHierarchy};
use crate::rx::{
    RxContainedRef, RxContainer, RxContainerGrouping, RxContainerItem, RxDatabase, RxEntry,
    RxGroup, RxGrouping, RxMetadata, RxTag, RxTemplate,
};

use super::KeepassRx;

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
