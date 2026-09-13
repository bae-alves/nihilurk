//! The flags, one row each.
//!
//! roog paints two things in stripes — the word the scorekeeper shouts when a
//! run doubles or a turn combos, and the log line that comes with it — and which
//! stripes those are is a table, not a constant. `-pride trans` swaps the flag
//! for the whole run and nothing else in the game notices: everything that
//! paints asks [`stripes`], and no other file names a flag.
//!
//! Adding one is a row in [`FLAGS`] and nothing else. Deliberately documented
//! nowhere but here — the switch is not in the manual, not in the CLI
//! reference, and not in `-h`. It is for whoever reads the source.
//!
//! ## About the colours
//!
//! A terminal has sixteen colours and a flag does not care. Each row below is
//! the closest honest reading of its flag in the palette roog actually has,
//! which means two compromises worth knowing about:
//!
//! * **Black stripes are [`Color::DarkGrey`]**, the terminal's "bright black".
//!   True black is the background, and a stripe you cannot see is not a stripe.
//! * **A symmetric flag repeats a colour where it wraps.** The stripes are
//!   cycled across the letters of a word, so a flag that reads the same
//!   backwards (trans, aro) puts two of the same colour together at the seam.
//!   That is the flag, not a bug — the rainbow is the only one that happens to
//!   wrap cleanly.

use bevy_ecs::prelude::*;
use crossterm::style::Color;

/// One flag: what to call it on the command line, and its stripes in order.
pub struct PrideFlag {
    /// The `-pride` argument that selects it.
    pub name: &'static str,
    /// Top to bottom, painted left to right across a word and cycled if the
    /// word is longer than the flag.
    pub stripes: &'static [Color],
}

/// Every flag roog can fly. The first row is the default.
#[rustfmt::skip]
pub const FLAGS: &[PrideFlag] = &[
    // The six-stripe rainbow. Six letters in DOUBLE, six letters in COMBO!,
    // six stripes — the one flag that wraps without repeating itself.
    PrideFlag { name: "pride",  stripes: &[
        Color::Red, Color::DarkYellow, Color::Yellow, Color::Green, Color::Blue, Color::DarkMagenta,
    ]},
    // Light blue, pink, white, pink, light blue.
    PrideFlag { name: "trans",  stripes: &[
        Color::Cyan, Color::Magenta, Color::White, Color::Magenta, Color::Cyan,
    ]},
    // Yellow, white, purple, black.
    PrideFlag { name: "nb",     stripes: &[
        Color::Yellow, Color::White, Color::DarkMagenta, Color::DarkGrey,
    ]},
    // Green, light green, white, grey, black.
    PrideFlag { name: "aro",    stripes: &[
        Color::DarkGreen, Color::Green, Color::White, Color::Grey, Color::DarkGrey,
    ]},
    // Black, grey, white, purple.
    PrideFlag { name: "ace",    stripes: &[
        Color::DarkGrey, Color::Grey, Color::White, Color::DarkMagenta,
    ]},
    // The lesbian sunset: dark orange, orange, white, pink, dark rose.
    PrideFlag { name: "sunset", stripes: &[
        Color::DarkYellow, Color::Red, Color::White, Color::Magenta, Color::DarkMagenta,
    ]},
];

impl PrideFlag {
    /// The flag `-pride <name>` asks for, or `None` if there is no such row —
    /// which is the caller's cue to say so and fly the default.
    pub fn named(name: &str) -> Option<&'static PrideFlag> {
        FLAGS.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The rainbow. The table is never empty, and its first row is the default.
    pub fn default_flag() -> &'static PrideFlag {
        &FLAGS[0]
    }
}

/// The flag this run is flying. Set once at startup from `-pride`; never
/// changes, never saved — it is a preference, not part of the run.
#[derive(Resource)]
pub struct Pride(pub &'static PrideFlag);

impl Default for Pride {
    fn default() -> Self {
        Self(PrideFlag::default_flag())
    }
}

/// The stripes to paint with: this run's flag, or the rainbow in a world that
/// never set one (every test world, and the engine before startup finishes).
pub fn stripes(world: &World) -> &'static [Color] {
    world
        .get_resource::<Pride>()
        .map_or(PrideFlag::default_flag(), |p| p.0)
        .stripes
}

/// Every flag's name, for a usage line that cannot go stale.
pub fn flag_names() -> Vec<&'static str> {
    FLAGS.iter().map(|f| f.name).collect()
}

/// What `-prideoff` prints, and the whole of what it does.
///
/// The flag is documented as turning the stripes off in favour of plain red.
/// It is not implemented, it will not be implemented, and the sentence below is
/// the entire feature. It is the one place in roog where the documentation and
/// the program disagree on purpose.
pub const PRIDE_OFF_REFUSAL: &str = "ERROR: You cannot ever take our pride.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flag_answers_to_the_name_it_prints() {
        for name in flag_names() {
            let flag = PrideFlag::named(name).expect("a row is found by its own name");
            assert_eq!(flag.name, name);
            assert!(!flag.stripes.is_empty(), "{name} has stripes to fly");
        }
    }

    #[test]
    fn an_unknown_name_is_no_flag_at_all() {
        assert!(PrideFlag::named("nope").is_none());
    }

    #[test]
    fn a_world_that_never_chose_flies_the_rainbow() {
        let w = World::new();
        assert_eq!(stripes(&w), PrideFlag::default_flag().stripes);
    }

    #[test]
    fn choosing_a_flag_changes_what_everything_paints_with() {
        let mut w = World::new();
        w.insert_resource(Pride(PrideFlag::named("trans").unwrap()));
        assert_eq!(stripes(&w), PrideFlag::named("trans").unwrap().stripes);
    }
}
