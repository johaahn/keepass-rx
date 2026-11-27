use qmetaobject::prelude::*;

use crate::rx::staged::StagedChange;

#[derive(Clone, Default, SimpleListItem)]
pub struct RxUiStagedChange {
    pub name: QString,
    pub custom: bool,
    pub kind: QString,
}

impl From<&StagedChange> for RxUiStagedChange {
    fn from(change: &StagedChange) -> Self {
        Self {
            name: QString::from(change.name.as_str()),
            custom: change.custom,
            kind: QString::from(change.kind.as_str()),
        }
    }
}
