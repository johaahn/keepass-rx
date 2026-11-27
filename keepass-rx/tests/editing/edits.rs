use crate::harness::*;
use keepass::db::EntryId;
use keepassrx::rx::{RxEdit, RxFieldEdit};

const TARGET: &str = "Full Entry";

fn full_entry() -> (TestVault, uuid::Uuid) {
    let vault = TestVault::from_fixture(FULL);
    let uuid = entry_uuid(&vault.reopen_raw(), TARGET);
    (vault, uuid)
}

#[test]
fn password_change_puts_the_old_password_last_in_history() {
    let (vault, uuid) = full_entry();
    let raw = vault.reopen_raw();
    let old = password_of(&raw, uuid);
    let before = history_passwords(&raw, uuid);
    vault.save_now(&password_edit(&raw, uuid, "second-pass"));

    let after = vault.reopen_raw();
    assert_eq!(password_of(&after, uuid), "second-pass");
    let history = history_passwords(&after, uuid);
    assert_eq!(history.len(), before.len() + 1);
    assert_eq!(history.last(), Some(&old));
}

#[test]
fn renaming_a_custom_field_keeps_its_protected_flag() {
    let (vault, uuid) = full_entry();
    let raw = vault.reopen_raw();
    let mut add = unchanged_edit(&raw, uuid);
    add.custom_fields.push(RxFieldEdit {
        original_name: None,
        name: "PIN".into(),
        value: Some(s("1234")),
        protected: true,
    });
    vault.save_now(&RxEdit::UpdateEntry(add));

    let raw = vault.reopen_raw();
    let mut rename = unchanged_edit(&raw, uuid);
    for field in &mut rename.custom_fields {
        if field.name == "PIN" {
            field.name = "Code".into();
        }
    }
    vault.save_now(&RxEdit::UpdateEntry(rename));

    let after = vault.reopen_raw();
    let entry = after.entry(EntryId::from_uuid(uuid)).expect("entry");
    assert!(!entry.fields.contains_key("PIN"));
    let code = entry.fields.get("Code").expect("renamed field");
    assert_eq!(code.get(), "1234");
    assert!(code.is_protected());
}

#[test]
fn unknown_entry_aborts_the_save() {
    let (vault, uuid) = full_entry();
    let raw = vault.reopen_raw();
    let mut edit = unchanged_edit(&raw, uuid);
    edit.uuid = uuid::Uuid::new_v4();
    let before = vault.bytes();
    let err = vault
        .save(
            &vault.source(),
            Some(&RxEdit::UpdateEntry(edit)),
            &keepassrx::rx::NoHooks,
        )
        .err()
        .expect("error");
    assert!(matches!(err, keepassrx::rx::SaveError::Edit(_)), "{err:?}");
    assert_eq!(vault.bytes(), before);
    assert_eq!(vault.listing(), vault.only_database());
}

fn hidden_values(xml: &str, keys: &[&str]) -> Vec<(String, String, Vec<String>)> {
    let doc = roxmltree::Document::parse(xml).expect("xml");
    let mut out = vec![];
    let current = doc
        .descendants()
        .filter(|n| n.has_tag_name("String"))
        .filter(|n| !n.ancestors().any(|a| a.has_tag_name("History")));
    for string in current {
        let child = |name: &str| string.children().find(|c| c.has_tag_name(name));
        let key = child("Key").and_then(|k| k.text()).unwrap_or_default();
        if keys.contains(&key) {
            let value = child("Value").expect("value");
            let attrs = value
                .attributes()
                .map(|a| format!("{}={}", a.name(), a.value()))
                .collect();
            let protected = value.attribute("Protected") == Some("True");
            let text = if protected {
                String::new()
            } else {
                value.text().unwrap_or_default().to_string()
            };
            out.push((key.to_string(), text, attrs));
        }
    }
    out.sort();
    out
}

#[test]
fn hidden_fields_survive_an_edit_unchanged() {
    let (vault, uuid) = full_entry();
    let mut raw = vault.reopen_raw();
    raw.config.version = keepass::config::DatabaseVersion::KDB4(1);
    raw.entry_mut(EntryId::from_uuid(uuid))
        .expect("entry")
        .edit(|e| {
            e.set_unprotected("_etm_template_uuid", "template-123");
            e.set_unprotected("KP2A_URL_1", "https://second.example.com");
            e.set_protected("KPEX_PASSKEY_PRIVATE_KEY_PEM", "pem-secret");
            e.set_unprotected("KeePassXC-Browser Settings", "{\"Allow\":[]}");
            e.set_unprotected("_LAST_MODIFIED", "Mon Jan 1 00:00:00 2024 GMT");
        });
    let mut bytes = vec![];
    raw.save(&mut bytes, key()).expect("serialize");
    std::fs::write(vault.path(), bytes).expect("write");

    let keys = [
        "_etm_template_uuid",
        "KP2A_URL_1",
        "KPEX_PASSKEY_PRIVATE_KEY_PEM",
        "KeePassXC-Browser Settings",
        "_LAST_MODIFIED",
    ];
    let xml_before = vault.xml();
    let before = hidden_values(&xml_before, &keys);
    assert_eq!(before.len(), keys.len());

    let raw = vault.reopen_raw();
    let mut edit = unchanged_edit(&raw, uuid);
    edit.password = Some(s("edited-pass"));
    edit.username = s("edited-user");
    vault.save_now(&RxEdit::UpdateEntry(edit));

    assert_eq!(hidden_values(&vault.xml(), &keys), before);
    assert_eq!(password_of(&vault.reopen_raw(), uuid), "edited-pass");
}

#[test]
fn ten_saves_in_a_row_grow_the_history_and_back_up_the_ninth_version() {
    let (vault, uuid) = full_entry();
    let raw = vault.reopen_raw();
    let start = history_passwords(&raw, uuid).len();
    let max = raw
        .meta
        .history_max_items
        .map_or(usize::MAX, |m| usize::try_from(m).expect("max"));
    let mut before_last = vec![];
    for n in 1..=10 {
        before_last = vault.bytes();
        let raw = vault.reopen_raw();
        vault.save_now(&password_edit(&raw, uuid, &format!("pass-{n}")));
    }
    assert_eq!(vault.backup_bytes(), before_last);

    let after = vault.reopen_raw();
    assert_eq!(password_of(&after, uuid), "pass-10");
    let history = history_passwords(&after, uuid);
    assert_eq!(history.len(), (start + 10).min(max));
    assert_eq!(history.last().map(String::as_str), Some("pass-9"));
}
