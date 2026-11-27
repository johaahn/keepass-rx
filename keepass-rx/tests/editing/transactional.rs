use crate::harness::*;
use keepass::config::DatabaseVersion;
use keepassrx::rx::{NoHooks, SaveError, SaveStep};
use std::fs;
use std::io::ErrorKind;

const TARGET: &str = "Full Entry";

fn prepared() -> (TestVault, uuid::Uuid, keepassrx::rx::RxEdit, Vec<u8>) {
    let vault = TestVault::from_fixture(FULL);
    let raw = vault.reopen_raw();
    let uuid = entry_uuid(&raw, TARGET);
    let edit = password_edit(&raw, uuid, "changed-pass");
    let before = vault.bytes();
    (vault, uuid, edit, before)
}

#[test]
fn failure_before_the_backup_changes_nothing() {
    for step in &ALL_STEPS[..=step_index(SaveStep::WriteBackup)] {
        let (vault, uuid, edit, before) = prepared();
        let src = vault.source();
        let err = vault
            .save(&src, Some(&edit), &FailAt(*step, ErrorKind::Other))
            .err()
            .expect("error");
        assert!(matches!(err, SaveError::Io(_)), "{step:?}: {err:?}");
        assert_eq!(vault.bytes(), before, "{step:?}");
        assert_eq!(vault.listing(), vault.only_database(), "{step:?}");
        assert_ne!(password_of(&vault.reopen_raw(), uuid), "changed-pass");
    }
}

#[test]
fn failure_at_the_database_write_keeps_the_original_and_the_backup() {
    for kind in [ErrorKind::Other, ErrorKind::StorageFull] {
        let (vault, uuid, edit, before) = prepared();
        let src = vault.source();
        let err = vault
            .save(&src, Some(&edit), &FailAt(SaveStep::WriteDatabase, kind))
            .err()
            .expect("error");
        match err {
            SaveError::Io(e) => assert_eq!(e.kind(), kind),
            other => panic!("{other:?}"),
        }
        assert_eq!(vault.bytes(), before);
        assert_eq!(vault.backup_bytes(), before);
        assert_eq!(vault.listing(), vault.database_and_backups());
        assert_ne!(password_of(&vault.reopen_raw(), uuid), "changed-pass");
    }
}

#[test]
fn failure_at_the_final_check_keeps_the_new_file_and_the_backup() {
    let (vault, uuid, edit, before) = prepared();
    let src = vault.source();
    let err = vault
        .save(
            &src,
            Some(&edit),
            &FailAt(SaveStep::VerifyFinal, ErrorKind::Other),
        )
        .err()
        .expect("error");
    assert!(matches!(err, SaveError::Io(_)), "{err:?}");
    assert_ne!(vault.bytes(), before);
    assert_eq!(vault.backup_bytes(), before);
    assert_eq!(vault.listing(), vault.database_and_backups());
    assert_eq!(password_of(&vault.reopen_raw(), uuid), "changed-pass");
    let (_, reopened) = vault.open();
    assert_ne!(reopened.sha256, src.sha256);
}

#[test]
fn a_change_before_each_step_up_to_the_recheck_is_a_conflict() {
    for step in &ALL_STEPS[..=step_index(SaveStep::RecheckOriginal)] {
        let (vault, _, edit, _) = prepared();
        let src = vault.source();
        let hook = MutateAt::new(*step, &vault);
        let err = vault.save(&src, Some(&edit), &hook).err().expect("error");
        assert!(matches!(err, SaveError::Conflict), "{step:?}: {err:?}");
        assert_eq!(vault.bytes(), hook.replacement, "{step:?}");
        assert_eq!(vault.listing(), vault.only_database(), "{step:?}");
    }
}

#[test]
fn change_after_the_recheck_is_kept_as_a_conflict_copy() {
    let (vault, uuid, edit, _) = prepared();
    let src = vault.source();
    let hook = MutateAt::new(SaveStep::SwapDatabase, &vault);
    let outcome = vault.save(&src, Some(&edit), &hook).expect("save");
    assert_eq!(password_of(&vault.reopen_raw(), uuid), "changed-pass");
    let copy = outcome.conflict_copy.expect("conflict copy");
    assert_eq!(copy.parent(), vault.path().parent());
    assert_eq!(fs::read(&copy).expect("read copy"), hook.replacement);
    assert_eq!(vault.listing().len(), 3);
}

#[test]
fn success_records_the_backup_and_the_new_hash() {
    let (vault, uuid, edit, before) = prepared();
    let src = vault.source();
    let outcome = vault.save(&src, Some(&edit), &NoHooks).expect("save");
    let after = vault.bytes();
    assert_eq!(outcome.sha256, keepassrx::rx::save::sha256_of(&after));
    assert_eq!(vault.source().sha256, outcome.sha256);
    assert_eq!(vault.backup_bytes(), before);
    assert_eq!(outcome.conflict_copy, None);
    assert_eq!(vault.listing(), vault.database_and_backups());
    assert_eq!(password_of(&outcome.db, uuid), "changed-pass");
    assert_eq!(password_of(&vault.reopen_raw(), uuid), "changed-pass");
    assert_eq!(vault.reopen_raw().config.version, DatabaseVersion::KDB4(1));
}

#[test]
fn a_stale_source_cannot_save_twice() {
    let (vault, _, edit, _) = prepared();
    let src = vault.source();
    vault.save(&src, Some(&edit), &NoHooks).expect("first save");
    let after_first = vault.bytes();
    let err = vault
        .save(&src, Some(&edit), &NoHooks)
        .err()
        .expect("error");
    assert!(matches!(err, SaveError::Conflict), "{err:?}");
    assert_eq!(vault.bytes(), after_first);
}
