use crate::harness::*;
use base64::{Engine, prelude::BASE64_STANDARD};
use keepass::db::EntryId;
use keepassrx::rx::RxEdit;
use keepassrx::rx::staged::{StagedEdit, open_staging_key, staging_key_path, write_staged};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const CLI: &str = "/usr/bin/keepassxc-cli";
const ENTRY: &str = "Test";

fn cli_available() -> bool {
    let present = Path::new(CLI).exists();
    if !present {
        eprintln!("keepassxc-cli not installed, skipping");
    }
    present
}

fn run(args: &[&str], db: &Path) -> String {
    cli(args, db, &[ENTRY])
}

fn cli(args: &[&str], db: &Path, rest: &[&str]) -> String {
    let mut child = Command::new(CLI)
        .args(args)
        .arg(db)
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(format!("{PASSWORD}\n").as_bytes())
        .expect("write password");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn show(db: &Path, attribute: &str) -> String {
    run(&["show", "-s", "-a", attribute], db)
}

#[test]
fn keepassxc_reads_the_edited_password() {
    if !cli_available() {
        return;
    }
    let vault = TestVault::from_fixture(V4_40);
    let raw = vault.reopen_raw();
    let uuid = entry_uuid(&raw, ENTRY);
    vault.save_now(&password_edit(&raw, uuid, "app-password"));
    assert_eq!(show(&vault.path(), "Password"), "app-password");
    assert_eq!(show(&vault.path(), "UserName"), "user");
}

#[test]
fn edits_alternate_between_the_app_and_keepassxc() {
    if !cli_available() {
        return;
    }
    let vault = TestVault::from_fixture(V4_40);
    let raw = vault.reopen_raw();
    let uuid = entry_uuid(&raw, ENTRY);
    vault.save_now(&password_edit(&raw, uuid, "first-app-pass"));

    run(&["edit", "-u", "kpxc-user"], &vault.path());
    let raw = vault.reopen_raw();
    let entry = raw.entry(EntryId::from_uuid(uuid)).expect("entry");
    assert_eq!(entry.get("UserName"), Some("kpxc-user"));
    assert_eq!(entry.get("Password"), Some("first-app-pass"));

    let (_, src) = vault.open();
    let edit = password_edit(&raw, uuid, "second-app-pass");
    vault
        .save(&src, Some(&edit), &keepassrx::rx::NoHooks)
        .expect("save");

    assert_eq!(show(&vault.path(), "UserName"), "kpxc-user");
    assert_eq!(show(&vault.path(), "Password"), "second-app-pass");
}

#[test]
fn keepassxc_opens_the_staging_key() {
    if !cli_available() {
        return;
    }
    let vault = TestVault::from_fixture(V4_40);
    let raw = vault.reopen_raw();
    let uuid = entry_uuid(&raw, ENTRY);
    let RxEdit::UpdateEntry(edit) = password_edit(&raw, uuid, "staged-pass");
    let src = vault.source();
    let staged = StagedEdit {
        edit,
        base_sha256: src.sha256,
        base: Default::default(),
        draft: false,
    };
    let path = vault.path().with_extension("edit");
    libsodium_rs::ensure_init().expect("libsodium");
    let staging =
        open_staging_key(&path, key(), &src.staging_config).expect("write staging key");
    write_staged(&path, &staging, &staged).expect("write staged");

    let csv = cli(&["export", "-f", "csv"], &staging_key_path(&path), &[]);
    assert!(
        csv.contains(&BASE64_STANDARD.encode(staging.as_bytes())),
        "{csv}"
    );
    assert!(!csv.contains("staged-pass"), "{csv}");
}
