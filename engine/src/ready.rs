//! The ready signal. With `NIHILURK_READY` set, the game writes [`READY`] the
//! moment it blocks on the player, so a script driving it through a pty (the
//! play agent's `serve.sh`) knows a key's effect is on screen without guessing
//! from a quiet spell. It is an OSC string, which a terminal that does not
//! know it ignores. Unset, nothing is written.
//!
//! It goes out on the same stream as the frame, after the frame's flush, so a
//! reader that sees it already has the frame it settles.

use std::io::Write;
use std::sync::OnceLock;

/// OSC, `nihilurk;ready`, BEL.
pub const READY: &[u8] = b"\x1b]nihilurk;ready\x07";

static ENABLED: OnceLock<bool> = OnceLock::new();

/// Whether `NIHILURK_READY` is set and not empty, read once.
fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("NIHILURK_READY").is_some_and(|v| !v.is_empty()))
}

/// Says the game now waits on the player. Called right before each read that
/// blocks for a key: the turn loop's, the travel cursor's and the death and
/// victory screens'. Not before a read that only lets a key skip an animation
/// or stop a walk: the game is not waiting there. Nor while a key is already
/// queued: the read takes it at once, and a signal then would come before that
/// key's frame, not after it. Off, it does not even look at the input.
pub fn signal_ready() -> std::io::Result<()> {
    let on = enabled();
    let key_pending = on && crate::input::waiting()?;
    write_ready(&mut std::io::stdout(), on, key_pending)
}

fn write_ready(out: &mut impl Write, on: bool, key_pending: bool) -> std::io::Result<()> {
    if on && !key_pending {
        out.write_all(READY)?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signal_is_written_and_flushed_only_when_on() {
        let mut out = Vec::new();
        write_ready(&mut out, false, false).unwrap();
        assert!(out.is_empty());
        write_ready(&mut out, true, false).unwrap();
        assert_eq!(out, READY);
    }
}
