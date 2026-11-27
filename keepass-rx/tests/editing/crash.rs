use crate::harness::*;
use keepassrx::rx::{NoHooks, SaveHooks, SaveStep, open::read_database, save_database};
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::Command;

const STEP_VAR: &str = "KPRX_CRASH_AT";
const VAULT_VAR: &str = "KPRX_CRASH_VAULT";
const TARGET: &str = "Full Entry";
const NEW_PASSWORD: &str = "changed-pass";
const SIGABRT: i32 = 6;

struct AbortAt(SaveStep);

impl SaveHooks for AbortAt {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step == self.0 {
            std::process::abort();
        }
        Ok(())
    }
}

fn step_named(name: &str) -> SaveStep {
    ALL_STEPS
        .into_iter()
        .find(|step| format!("{step:?}") == name)
        .unwrap_or_else(|| panic!("unknown step {name}"))
}

#[test]
fn crash_child() {
    let (Ok(step), Ok(path)) = (std::env::var(STEP_VAR), std::env::var(VAULT_VAR)) else {
        return;
    };
    let path = PathBuf::from(path);
    let (db, src) = read_database(&path, key()).expect("open");
    let uuid = entry_uuid(&db, TARGET);
    let edit = password_edit(&db, uuid, NEW_PASSWORD);
    let backup = path
        .parent()
        .expect("parent")
        .join(BACKUP_DIR)
        .join("db.bak");
    let _ = save_database(
        &src,
        key(),
        &backup,
        |db| edit.apply(db).map_err(Into::into),
        |_| Ok(()),
        &AbortAt(step_named(&step)),
    );
    std::process::exit(0);
}

#[test]
fn a_crash_at_any_step_leaves_an_openable_database() {
    for step in ALL_STEPS {
        let vault = TestVault::from_fixture(FULL);
        let before = vault.bytes();
        let raw = vault.reopen_raw();
        let uuid = entry_uuid(&raw, TARGET);
        let old_password = password_of(&raw, uuid);

        let status = Command::new(std::env::current_exe().expect("current exe"))
            .args(["--exact", "crash::crash_child", "--test-threads=1"])
            .env(STEP_VAR, format!("{step:?}"))
            .env(VAULT_VAR, vault.path())
            .status()
            .expect("spawn child");
        assert_eq!(status.signal(), Some(SIGABRT), "{step:?}: {status:?}");

        let after = vault.reopen_raw();
        let password = password_of(&after, uuid);
        assert!(
            password == old_password || password == NEW_PASSWORD,
            "{step:?}: {password}"
        );
        if step_index(step) <= step_index(SaveStep::SwapDatabase) {
            assert_eq!(vault.bytes(), before, "{step:?}");
            assert_eq!(password, old_password, "{step:?}");
        } else {
            assert_eq!(password, NEW_PASSWORD, "{step:?}");
        }
        vault
            .save(&vault.source(), None, &NoHooks)
            .expect("save after crash");
    }
}
