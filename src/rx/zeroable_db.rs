use std::ops::Deref;

use keepass::{
    Database,
    db::{Entry, Group, NodeRefMut, Value},
};
use take_mut::take;
use zeroize::Zeroize;

/// Wrapper struct for zeroing out a loaded database file as much as
/// possible, on top of what the KeePass library already does.
#[allow(dead_code)]
pub struct ZeroableDatabase(pub Database);

impl Deref for ZeroableDatabase {
    type Target = Database;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! zero_out {
    ($value:expr) => {{
        $value.zeroize();
        $value
    }};
}

fn zero_group(group: &mut Group) {
    for node in group.children.iter_mut() {
        match node.as_mut() {
            NodeRefMut::Group(group) => zero_group(group),
            NodeRefMut::Entry(entry) => zero_entry(entry),
        }
    }

    for (_, data_item) in group.custom_data.items.iter_mut() {
        if let Some(value) = &mut data_item.value {
            zero_value(value);
        }
    }

    take(&mut group.name, |mut name| zero_out!(name));
}

fn zero_entry(entry: &mut Entry) {
    for (_, value) in entry.fields.iter_mut() {
        zero_value(value);
    }
}

fn zero_value(value: &mut Value) {
    match value {
        Value::Bytes(bytes) => take(bytes, |mut bytes| zero_out!(bytes)),
        Value::Protected(value) => value.zero_out(),
        Value::Unprotected(value) => take(value, |mut value| zero_out!(value)),
    }
}

impl Zeroize for ZeroableDatabase {
    fn zeroize(&mut self) {
        let db = &mut self.0;
        zero_group(&mut db.root);
    }
}
