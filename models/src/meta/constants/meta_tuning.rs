//! Tuning for score, the HUD and screen shake.

// ===========================================================================
// Score
// ===========================================================================

/// What the number on the HUD is made of. The verbs are in `score.rs`, which
/// documents the whole table in one place.
pub mod score {
    /// Score paid per point of a slain creature's `max_hp`. A kestral is a
    /// rounding error next to a griffin, which is the intent: the scoreboard
    /// rewards fighting things that could have killed you.
    pub const KILL_PER_MAX_HP: i32 = 100;

    /// What each corpse past the first adds to a turn's kill score, as a
    /// fraction of the whole pile: the turn's kills are multiplied by one plus
    /// this for each extra corpse, applied to the kills together, not to the
    /// last one alone. This is the dial that decides whether a thrown wand is worth more
    /// than the same six kills one at a time.
    pub const COMBO_BONUS_PER_KILL: f32 = 0.5;

    /// How often a combo is logged as done "With pride." instead of the usual
    /// "With style." Rare on purpose: a line that shows up every other fight
    /// stops being one.
    pub const COMBO_PRIDE_CHANCE: f64 = 0.05;

    /// Score paid per difficulty tier every time a staircase is used, counting
    /// the shallowest band as tier one so the first flight still pays. Raise it
    /// and diving outscores clearing; lower it and the reverse.
    pub const STAIR_PER_TIER: i32 = 500;

    /// Turns a payment stays lit on the scorekeeper. The default is one full
    /// frame of screen time: the flash is aged at the tail of the turn it was armed in,
    /// shown by that turn's render, and dark by the player's next action.
    pub const SCORE_FLASH_TURNS: u8 = 2;

    /// The kill-score multiplier a creature carrying
    /// [`crate::effects::ScoreBounty`] pays out — the apis guarding a
    /// treasure hive is worth several ordinary rattlesnakes.
    pub const BOUNTY_SCORE_MULTIPLIER: i32 = 5;
}

// ===========================================================================
// HUD / message log
// ===========================================================================

/// Sizing for the on-screen log. These are display, not balance, but they live
/// here because they are the kind of number people fiddle with and they are
/// coupled to [`crate::constants::map::WIDTH`].
pub mod hud {
    /// Message-log lines shown at the bottom of the screen at once. More lines
    /// means less playfield unless the terminal is tall.
    pub const LOG_LINES: usize = 2;

    /// Column the command bar's text starts at. The bar fills the screen's
    /// bottom three rows and leaves two blank columns before [`LOG_X`], so its
    /// lines hold up to `LOG_X - 3` characters (the longest, Spanish, is 23).
    pub const BAR_X: u16 = 1;

    /// Column the log and the player line start at: right of the command bar
    /// and its divider. Everything to its left on the bottom three rows is
    /// the bar.
    pub const LOG_X: u16 = 26;

    /// Wrap width for a log line once the command bar is hidden: the whole
    /// screen, which is the map's width.
    pub const LOG_FULL_WIDTH: usize = crate::constants::map::WIDTH as usize;

    /// Wrap width for a log line in the normal view: the screen's width less
    /// the command bar.
    pub const LOG_WIDTH: usize = LOG_FULL_WIDTH - LOG_X as usize;

    /// Columns the "-- more --" prompt takes from the last log line: its 22
    /// plus a space. The pager wraps that line this much narrower.
    pub const MORE_PROMPT_WIDTH: usize = 23;

    /// How many past message-log lines the scrollback keeps before the oldest
    /// falls off.
    pub const LOG_HISTORY_CAP: usize = 50;
}

// ===========================================================================
// Screen shake
// ===========================================================================

/// How long each kind of shake rocks the map, and how far its first frame
/// throws it. See [`crate::shake::ShakeKind`] for what arms each one, and for
/// why none may last less than two animation frames.
pub mod shake {
    /// How long the player's own landed blow shakes the map, in ms. The floor:
    /// two shaken frames is the least a shake can be and still be one.
    pub const HIT_MS: f32 = 80.0;

    /// Cells the map is thrown on a hit's opening frame.
    pub const HIT_AMPLITUDE: i8 = 1;

    /// How long a death in sight shakes the map, in ms.
    pub const KILL_MS: f32 = 120.0;

    /// Cells the map is thrown on a kill's opening frame.
    pub const KILL_AMPLITUDE: i8 = 1;

    /// How long a blast in sight, or the player's own excellent hit, shakes
    /// the map, in ms.
    pub const HEAVY_MS: f32 = 260.0;

    /// Cells the map is thrown on a heavy shake's opening frame.
    pub const HEAVY_AMPLITUDE: i8 = 2;

    /// How long the room reels when the player is knocked down through the
    /// low-HP warning, in ms.
    pub const WOUNDED_MS: f32 = 460.0;

    /// Cells the map is thrown on a wounded shake's opening frame.
    pub const WOUNDED_AMPLITUDE: i8 = 2;
}
