//! UI and input state kept as resources: the menus (spells, help, item info), the quit
//! prompt, the command bar, targeting, and the player's name. None of it changes the rules.

use super::*;

// ===========================================================================
// UI and input state (resources)
// ===========================================================================

/// The name the player typed at the start of the run, shown on the death screen.
#[derive(Resource, Default)]
pub struct PlayerName {
    /// The name the player typed.
    pub what: String,
}

// The pack screen's own state — `ItemAction`, `PackMode` and `PackIsOpen` —
// lives in [`crate::pack`], next to the row filtering that decides what each of
// its ten modes shows.

/// Whether the `Z` spells menu is open, and which slot the cursor sits on (one
/// per [`SPELLSET_CAP`](crate::constants::spells::SPELLSET_CAP)). Picking a row — by its letter, or by navigating and confirming
/// — opens the aiming reticle on that spell exactly the way the pack's `Use`
/// row does on an item. This menu is the only way to an active spell; no key
/// fires a slot directly.
#[derive(Resource, Default)]
pub struct SpellsMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The slot the cursor sits on.
    pub selected: usize,
}

/// Whether the "Really quit?" prompt is up.
///
/// Quitting is the one irreversible thing a keystroke can do — the run is
/// written out and the terminal goes away — so `Q` and `X` ask first, and a
/// misfire costs a `n` instead of a session. Ctrl+C is deliberately *not*
/// routed through here: it is the shell's own kill, a player reaching for it
/// means it, and a program that argues with Ctrl+C is a program you have to
/// kill twice.
#[derive(Resource, Default)]
pub struct QuitPrompt {
    /// Whether the prompt is up.
    pub open: bool,
}

/// The F1 key list: whether it is up. Any key closes it and does nothing else,
/// so it can never cost a turn.
#[derive(Resource, Default)]
pub struct HelpMenu {
    /// Whether the list is up.
    pub open: bool,
}

/// The pack's Info popup: whether it is up and the lines it shows. Any key
/// closes it and does nothing else, so it can never cost a turn.
#[derive(Resource, Default)]
pub struct ItemInfo {
    /// Whether the popup is up.
    pub open: bool,
    /// What it shows, the item's name first.
    pub lines: Vec<String>,
}

/// The yellow command bar at the bottom left: whether the player has opted out
/// of it with F2. Hiding it hands its columns to the log and the player line
/// (see `CommandBar::log_x` and `log_width` in `hud`).
#[derive(Resource, Default)]
pub struct CommandBar {
    /// Whether the bar is hidden.
    pub hidden: bool,
}

/// The aiming reticle: which item is being aimed, whether this is a throw or a
/// zap, and where the cursor is.
#[derive(Resource, Default)]
pub struct TargetingState {
    /// Whether the reticle is up.
    pub active: bool,
    /// The item being thrown or zapped. `None` for a spell, a look, or a reach
    /// strike.
    pub item: Option<Entity>,
    /// The reticle is aiming a throw rather than a zap: the range is
    /// [`crate::items::THROW_RANGE`] instead of the item's own, and confirming
    /// hurls the item instead of using it.
    pub throwing: bool,
    /// The reticle is triggering an active spell rather than an item —
    /// `item` is `None` whenever this is `Some`. Confirming queues a
    /// [`WantsToCast`] instead of a [`WantsToUse`].
    pub spell_effect: Option<&'static crate::catalog::SpellDef>,
    /// The reticle is a plain look: nothing is queued and no turn is spent —
    /// confirming only logs what's on the aimed tile. `item` and `spell_effect`
    /// are both `None` whenever this is set, and unlike every other reticle
    /// purpose it may be confirmed on the player's own tile.
    pub looking: bool,
    /// The reticle is a reach weapon's strike (a bardiche, a whip) rather than
    /// a thrown or zapped item: its range is the wielded weapon's own
    /// [`Reach`], and confirming resolves the strike in place — no item ever
    /// leaves the wielder's hand. See `crate::combat::resolve_reach_attack`.
    pub reach_attack: bool,
    /// The cursor's column, as a map tile coordinate.
    pub cursor_x: i16,
    /// The cursor's row, as a map tile coordinate.
    pub cursor_y: i16,
}
