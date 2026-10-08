//! Every tuning knob in the binary, in one place.
//!
//! The balance of the game lives in `models/src/constants.rs`. This file is its
//! counterpart for what the *terminal* does with a frame: how big the screen
//! is, how long a frame is held, how long a turn the player did not choose
//! stays on screen.
//!
//! Each constant is defined here and re-exported from the module that uses it,
//! so call sites keep working. Change the value here; nothing else needs to
//! move unless a doc comment below says so.
//!
//! ## What is *not* here
//!
//! * `STUMBLE_DIRS` in `models/src/player.rs` and `SUPPORTED` in `bin/dispatch.rs`.
//!   Structural tables, not knobs. The dispatcher is also its own crate root and
//!   shares no module with this binary.
//! * `BLANK_CELL` in `screen.rs`. It is a value of a type that module owns.

/// The cell grid the game paints onto.
pub mod screen {
    /// Screen width in columns. The classic 80x25.
    pub const SCREEN_W: u16 = 80;

    /// Screen height in rows: row 0 is the status line, rows 1..=22 hold the
    /// map, rows 22..25 hold the message log. The map rows are
    /// `models::MAP_HEIGHT` of them, so change one and you change the other.
    pub const SCREEN_H: u16 = 25;

    /// The screen row map row 0 paints on — row 0 being the status line.
    ///
    /// Every map-space painter folds this in, which is why `render`'s map layers
    /// pass a raw map coordinate instead of each carrying its own `y + 1`.
    pub const MAP_TOP: u16 = 1;
}

/// How long the game holds a frame, and how long it pauses between turns.
pub mod timing {
    /// One animation frame, in ms, before `-anim-rate` scales it and
    /// [`ANIMATION_SLOWDOWN`] stretches it. Particles, shakes and magic-map
    /// wipes all step on this clock, so they stay in lockstep.
    pub const BASE_FRAME_MS: u64 = 33;

    /// A multiplier on how long the loop *sleeps* between animation frames,
    /// independently of `AnimRate`: it stretches only the real time between
    /// redraws, leaving how far the simulated clock (and every particle's own
    /// keyframe timing) advances per frame untouched. So every blast, spark,
    /// flight, shake and magic-map wipe plays out the same sequence of frames it
    /// always did, just held a beat longer — one knob, not a retune of each
    /// effect's own duration constants.
    pub const ANIMATION_SLOWDOWN: f32 = 4.0 / 3.0 * 1.1;

    /// How long a turn the player lost to paralysis, or spent asleep in gas,
    /// holds the screen, so the monsters' free move is something the player
    /// watches happen rather than finds already done.
    pub const INCAPACITATED_PAUSE_MS: u64 = 90;

    /// How long each step of an auto-explore holds the screen, so the walk
    /// reads as a walk instead of a teleport.
    pub const AUTOEXPLORE_STEP_MS: u64 = 35;

    /// How long the travel cursor waits for a key before it flips its blink.
    pub const TRAVEL_BLINK_MS: u64 = 400;
}
