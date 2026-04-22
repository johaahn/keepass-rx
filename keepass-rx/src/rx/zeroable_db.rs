use std::ops::{Deref, DerefMut};

use keepass::{
    Database,
    db::{CustomDataValue, Entry, Group, Value},
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

impl DerefMut for ZeroableDatabase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

macro_rules! zero_out {
    ($value:expr) => {{
        $value.zeroize();
        $value
    }};
}

fn zero_group(group: &mut Group) {
    for entry in group.entries.iter_mut() {
        zero_entry(entry);
    }

    for group in group.groups.iter_mut() {
        zero_group(group);
    }

    for (_, data_item) in group.custom_data.iter_mut() {
        if let Some(value) = &mut data_item.value {
            zero_custom_data_value(value);
        }
    }

    take(&mut group.name, |mut name| zero_out!(name));
}

fn zero_entry(entry: &mut Entry) {
    for (_, value) in entry.fields.iter_mut() {
        zero_value(value);
    }
}

fn zero_custom_data_value(value: &mut CustomDataValue) {
    match value {
        CustomDataValue::Binary(bytes) => take(bytes, |mut bytes| zero_out!(bytes)),
        CustomDataValue::String(value) => value.zeroize(),
    }
}

fn zero_value<T>(value: &mut Value<T>)
where
    T: Zeroize,
{
    match value {
        Value::Protected(value) => value.zeroize(),
        Value::Unprotected(value) => take(value, |mut value| zero_out!(value)),
    }
}

impl Zeroize for ZeroableDatabase {
    fn zeroize(&mut self) {
        let db = &mut self.0;
        zero_group(&mut db.root);
    }
}
