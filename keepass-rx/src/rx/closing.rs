use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use super::save::{SaveHooks, SaveStep};

pub static APP_CLOSING: AtomicBool = AtomicBool::new(false);

pub struct StopWhenClosing<'a>(pub &'a AtomicBool);

impl SaveHooks for StopWhenClosing<'_> {
    fn before(&self, step: SaveStep) -> io::Result<()> {
        if step <= SaveStep::WriteDatabase && self.0.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "the app is closing",
            ));
        }
        Ok(())
    }
}
