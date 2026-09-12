//! Screen shake: the whole map lurching a cell or two off its moorings when
//! something hits hard enough to deserve it.
//!
//! Kin to [`crate::particles`] in every way that matters — transient, never
//! saved, no gameplay rides on it, gameplay code just arms it and forgets — and
//! deliberately unlike it in one. A particle batch *blocks*: the turn is
//! already resolved, so the engine can freeze for 200 ms and play it out the
//! way NetHack freezes for a bolt. A shake must not. It is armed by things that
//! happen while you are in the middle of fighting for your life, which is
//! exactly when a player is typing ahead, and a juicy effect that costs them a
//! keystroke stops being juicy the second time it happens. So the engine plays
//! a shake only in the gaps where nothing is waiting to be read, and the
//! instant a key arrives it [`Shake::settle`]s and hands the key back unread —
//! see `play_shake` in `engine/src/view.rs`.
//!
//! The shake never changes what the game can see. It displaces the *map layers*
//! within the fixed 80x25 frame; the frame itself, the status line, the message
//! log and the pack overlay do not move, and a tile the displacement pushes past
//! the edge of the map viewport is simply not drawn that frame. There is no
//! second viewport, no widened field of view, and nothing off-screen is rendered
//! to fill the gap — the map is the same size mid-shake as it is at rest.
//!
//! The arithmetic — which way the map is thrown on a given frame, and how far —
//! is [`particle_core::shake_offset`], for the same reason the rest of the
//! effect layer's timing lives there: it is pure, it allocates nothing, and
//! `compat/` compiles it for a microcontroller on every run.

use bevy_ecs::prelude::*;

use particle_core as core_math;

/// What is doing the shaking. Each kind is one row of the table in
/// [`ShakeKind::shape`]: how long it rocks for, and how far it throws the map
/// on the first frame.
///
/// Four kinds is the whole set, and that is the point — a shake is a
/// punctuation mark. Give one to every hit and it stops meaning anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShakeKind {
    /// Something died where the player could see it. A single sharp tick of
    /// recoil — short, because on a good floor it fires several times a
    /// minute, and anything longer would have the map permanently trembling.
    Kill,
    /// The heavy one: a real thump that decays, armed by the two things that
    /// hit hardest — a blast going off in sight, and the player's own
    /// excellent hit. Named for the weight, not the cause, because it now has
    /// two of them.
    Heavy,
    /// The player was just knocked down through the low-HP warning. Long, and
    /// the only one that is *not* about the blow that caused it — it is the
    /// room reeling, under a log line the player needs to read, so it runs on
    /// well past the hit and fades out rather than stopping.
    Wounded,
    /// The player died. The biggest and last thing the map ever does: it is
    /// the end of the run, there is no keystroke left to protect, and the
    /// death screen is coming in behind it. Nothing outranks this one.
    Death,
}

impl ShakeKind {
    /// `(duration_ms, amplitude_in_cells)`. The per-kind dials, on the kind
    /// itself rather than off in `constants.rs`: there are four of them, they
    /// are only ever read here, and "short / medium / long / final" is only
    /// legible as a table if you can see all four pairs at once.
    ///
    /// Amplitude 2 means the first frames throw the map two cells and the rest
    /// one — a shake that decays in *reach*, since a terminal has no half-cell
    /// to decay through. Amplitude 1 is a shake that only decays in time.
    const fn shape(self) -> (f32, i8) {
        match self {
            ShakeKind::Kill => (120.0, 1),
            ShakeKind::Heavy => (260.0, 2),
            ShakeKind::Wounded => (460.0, 2),
            ShakeKind::Death => (900.0, 3),
        }
    }

    /// How long this kind rocks for, in ms.
    pub const fn duration_ms(self) -> f32 {
        self.shape().0
    }

    /// How far this kind throws the map on its opening frame, in cells.
    pub const fn amplitude(self) -> i8 {
        self.shape().1
    }
}

/// The shake layer: at most one shake at a time, plus the switch that turns the
/// whole feature off.
///
/// Gameplay code only ever calls [`Shake::kick`]. The engine owns everything
/// else — ageing it a frame at a time, reading the displacement, settling it
/// when the player acts.
#[derive(Resource)]
pub struct Shake {
    /// What is shaking, or `None` at rest.
    kind: Option<ShakeKind>,
    /// Milliseconds since this shake was armed.
    age_ms: f32,
    /// `false` under `-nshake`: [`Shake::kick`] becomes a no-op and the map
    /// never leaves its moorings. Motion on a terminal is not free for
    /// everyone, and `-anim-rate` can only make a shake *slower*, which is the
    /// wrong direction for someone who wants none.
    pub enabled: bool,
}

impl Default for Shake {
    fn default() -> Self {
        Self {
            kind: None,
            age_ms: 0.0,
            enabled: true,
        }
    }
}

impl Shake {
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm a shake, if it is worth more than whatever is already running.
    ///
    /// The comparison is against the *remaining* time, not the running shake's
    /// full length, and that is what makes the rule feel right in the two cases
    /// it exists for. A kill landed in the middle of a blast's thump does not
    /// cut that thump down to 120 ms. The same kill landed as the thump
    /// finishes extends it, because by then there is less left of the blast
    /// than the kill is worth.
    pub fn kick(&mut self, kind: ShakeKind) {
        if !self.enabled {
            return;
        }
        if self.remaining_ms() > kind.duration_ms() {
            return;
        }
        self.kind = Some(kind);
        self.age_ms = 0.0;
    }

    /// Age the shake by one frame, retiring it once it is spent.
    pub fn advance(&mut self, dt_ms: f32) {
        let Some(kind) = self.kind else {
            return;
        };
        self.age_ms += dt_ms;
        if self.age_ms >= kind.duration_ms() {
            self.settle();
        }
    }

    /// Stop dead and put the map back where it belongs. The engine calls this
    /// the moment the player presses a key: input wins, always, and the frame
    /// they act on is never a skewed one.
    pub fn settle(&mut self) {
        self.kind = None;
        self.age_ms = 0.0;
    }

    /// Whether the map is currently off its moorings.
    pub fn active(&self) -> bool {
        self.kind.is_some()
    }

    /// How much of the running shake is left, in ms; `0.0` at rest.
    pub fn remaining_ms(&self) -> f32 {
        self.kind
            .map_or(0.0, |k| (k.duration_ms() - self.age_ms).max(0.0))
    }

    /// Where the map sits this frame, in whole cells right and down of where it
    /// belongs. `(0, 0)` at rest.
    pub fn offset(&self) -> (i16, i16) {
        let Some(kind) = self.kind else {
            return (0, 0);
        };
        let (dx, dy) = core_math::shake_offset(self.age_ms, kind.duration_ms(), kind.amplitude());
        (dx as i16, dy as i16)
    }
}

/// Arm a shake on a world that may not have the resource at all.
///
/// Every caller is gameplay code, and gameplay code runs in tests that build a
/// bare world with no effect layer in it — the same reason
/// [`crate::particles::Particles`] is reached through `get_resource_mut` at its
/// call sites. Doing it once here keeps that concern out of combat and item
/// code, which should not have to know the renderer exists.
pub fn kick_shake(world: &mut World, kind: ShakeKind) {
    if let Some(mut shake) = world.get_resource_mut::<Shake>() {
        shake.kick(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_shake_is_at_rest() {
        let shake = Shake::new();
        assert!(!shake.active());
        assert_eq!(shake.offset(), (0, 0));
    }

    #[test]
    fn a_kick_displaces_the_map_and_running_out_puts_it_back() {
        let mut shake = Shake::new();
        shake.kick(ShakeKind::Heavy);
        assert!(shake.active());
        assert_ne!(shake.offset(), (0, 0));

        shake.advance(ShakeKind::Heavy.duration_ms());
        assert!(!shake.active(), "a spent shake retires itself");
        assert_eq!(shake.offset(), (0, 0));
    }

    #[test]
    fn a_bigger_shake_is_not_cut_short_by_a_smaller_one() {
        let mut shake = Shake::new();
        shake.kick(ShakeKind::Heavy);
        shake.advance(30.0);
        shake.kick(ShakeKind::Kill);
        assert!(
            shake.remaining_ms() > ShakeKind::Kill.duration_ms(),
            "a crit mid-blast truncated the blast"
        );
    }

    #[test]
    fn a_smaller_shake_extends_a_bigger_one_that_is_nearly_spent() {
        let mut shake = Shake::new();
        shake.kick(ShakeKind::Heavy);
        shake.advance(ShakeKind::Heavy.duration_ms() - 10.0);
        shake.kick(ShakeKind::Kill);
        assert_eq!(shake.remaining_ms(), ShakeKind::Kill.duration_ms());
    }

    #[test]
    fn settling_stops_it_dead() {
        let mut shake = Shake::new();
        shake.kick(ShakeKind::Wounded);
        shake.settle();
        assert!(!shake.active());
        assert_eq!(shake.offset(), (0, 0));
    }

    #[test]
    fn disabled_never_leaves_the_moorings() {
        let mut shake = Shake::new();
        shake.enabled = false;
        for kind in [
            ShakeKind::Kill,
            ShakeKind::Heavy,
            ShakeKind::Wounded,
            ShakeKind::Death,
        ] {
            shake.kick(kind);
            assert!(!shake.active(), "{kind:?} shook with -nshake set");
        }
    }

    #[test]
    fn short_medium_long_stay_in_that_order() {
        // The one property the request was actually written in terms of, with
        // death on the end of it: the last shake of a run is the biggest.
        assert!(ShakeKind::Kill.duration_ms() < ShakeKind::Heavy.duration_ms());
        assert!(ShakeKind::Heavy.duration_ms() < ShakeKind::Wounded.duration_ms());
        assert!(ShakeKind::Wounded.duration_ms() < ShakeKind::Death.duration_ms());
        assert!(ShakeKind::Death.amplitude() > ShakeKind::Wounded.amplitude());
    }

    #[test]
    fn death_outranks_everything_already_rocking() {
        let mut shake = Shake::new();
        shake.kick(ShakeKind::Wounded);
        shake.kick(ShakeKind::Death);
        assert_eq!(shake.remaining_ms(), ShakeKind::Death.duration_ms());
    }

    #[test]
    fn kicking_a_world_without_the_resource_is_harmless() {
        let mut world = World::new();
        kick_shake(&mut world, ShakeKind::Heavy);
    }
}
