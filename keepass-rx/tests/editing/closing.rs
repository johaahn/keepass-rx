use crate::harness::*;
use keepassrx::rx::closing::StopWhenClosing;
use keepassrx::rx::{SaveError, SaveHooks, SaveStep};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

const TARGET: &str = "Full Entry";

struct CloseAt<'a> {
    step: SaveStep,
    closing: &'a AtomicBool,
}

impl SaveHooks for CloseAt<'_> {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step == self.step {
            self.closing.store(true, Ordering::SeqCst);
        }
        StopWhenClosing(self.closing).before(step)
    }
}

#[test]
fn closing_stops_every_step_up_to_the_database_write() {
    for step in &ALL_STEPS[..=step_index(SaveStep::WriteDatabase)] {
        let vault = TestVault::from_fixture(FULL);
        let raw = vault.reopen_raw();
        let uuid = entry_uuid(&raw, TARGET);
        let edit = password_edit(&raw, uuid, "changed-pass");
        let before = vault.bytes();
        let closing = AtomicBool::new(false);

        let err = vault
            .save(
                &vault.source(),
                Some(&edit),
                &CloseAt {
                    step: *step,
                    closing: &closing,
                },
            )
            .err()
            .expect("error");

        assert!(
            matches!(&err, SaveError::Io(io) if io.kind() == io::ErrorKind::Interrupted),
            "{step:?}: {err:?}"
        );
        assert_eq!(vault.bytes(), before, "{step:?}");
        assert!(
            vault.listing().is_subset(&vault.database_and_backups()),
            "{step:?}: {:?}",
            vault.listing()
        );
    }
}

#[test]
fn closing_after_the_swap_started_finishes_the_save() {
    for step in [SaveStep::SwapDatabase, SaveStep::VerifyFinal] {
        let vault = TestVault::from_fixture(FULL);
        let raw = vault.reopen_raw();
        let uuid = entry_uuid(&raw, TARGET);
        let edit = password_edit(&raw, uuid, "changed-pass");
        let closing = AtomicBool::new(false);

        vault
            .save(
                &vault.source(),
                Some(&edit),
                &CloseAt {
                    step,
                    closing: &closing,
                },
            )
            .expect("save");

        assert_eq!(
            password_of(&vault.reopen_raw(), uuid),
            "changed-pass",
            "{step:?}"
        );
    }
}
