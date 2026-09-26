//! What the player *is* — the one module that answers it.
//!
//! Three answers, and the same three wherever the question is asked
//! ([`Body`]): **nihil**, the default; **lurk**, the other one written to be
//! played; or a **bestiary row**, for `-am <species>`. They are one enum
//! rather than two flags because they are one choice: you are exactly one of
//! them, and the argument parser has nothing to cross-check.
//!
//! Everything downstream that has to know what a creature is made of comes
//! through here: what tempo a staircase returns them to ([`innate_tempo`]),
//! and whether they have the hands to put something on ([`equip_refusal`]).
//!
//! ## How a form survives a save
//!
//! Neither class nor species costs the save file a field, which matters:
//! `crate::saveload` is not versioned, and every field it has ever gained has
//! killed every run already in progress.
//!
//! * A **monster body** is read back from the player's [`Name`], which *is*
//!   the species (see [`crate::saveload`]), as the [`MonsterBody`] marker.
//! * A **lurk** is read back from the [`Lurk`] marker, which rides the effect
//!   ledger like any other thing a creature was born with.
//! * **nihil** is what a player with neither is.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;

use crate::components::*;
use crate::constants::lurk;
use crate::effects::{BuildsMomentum, Grant, Lunges, Lurk, Stealthy, grant_all};
use crate::equipment::Slot;
use crate::map::GameRng;
use crate::monsters::MonsterDef;

/// What a player is. One of these, never two.
#[derive(Clone, Copy, Default)]
pub enum Body {
    /// The default. Wakes up human-shaped and carrying a mace.
    #[default]
    Nihil,
    /// Quadruped, fanged, clawed, furred: its own species, and the only one
    /// besides nihil that was written to be played. See [`wear_lurk`].
    Lurk,
    /// Any bestiary row, worn as a costume — `-am dragon`. See
    /// [`crate::monsters::wear_monster`].
    Monster(&'static MonsterDef),
}

impl Body {
    /// The name a form answers to on the command line, and in an error about
    /// one.
    pub fn name(self) -> &'static str {
        match self {
            Body::Nihil => "nihil",
            Body::Lurk => "lurk",
            Body::Monster(def) => def.name,
        }
    }

    /// Whether this form starts out carrying the usual kit. Only nihil does:
    /// a mace is no use to a creature with paws, and a monster is not stocked
    /// the way a floor stocks one.
    pub fn brings_a_pack(self) -> bool {
        matches!(self, Body::Nihil)
    }
}

/// Which body the *next* player spawned wakes up in. Read once, by
/// [`crate::map::initialize_world`].
///
/// A resource for the same reason [`PlayerName`] is one: the player is built
/// inside map generation, well below the argument parser, and the alternative
/// is dressing them up again afterwards — which would mean spawning the
/// starting pack only to take it away.
#[derive(Resource, Default)]
pub struct StartingBody(pub Body);

/// Puts `body` on the freshly spawned `player`. A no-op for nihil, who is
/// what the spawn already built.
pub fn wear(world: &mut World, player: Entity, body: Body) {
    match body {
        Body::Nihil => {}
        Body::Lurk => wear_lurk(world, player),
        Body::Monster(def) => crate::monsters::wear_monster(world, player, def),
    }
}

/// What a lurk is born with. A `const` slice because [`grant_all`] takes the
/// same `&'static [Grant]` a bestiary row hands it — one shape for innate
/// magic, whatever is carrying it.
const LURK_GRANTS: &[Grant] = &[
    Grant::of::<Lurk>(),
    Grant::of::<Lunges>(),
    Grant::of::<BuildsMomentum>(),
    Grant::of::<Stealthy>(),
];

/// The lurk: quadruped, fanged, clawed, furred, and magenta.
///
/// It brings nothing and can never wear a weapon or a suit of armour — the
/// claws *are* the weapon and the fur *is* the armour, which is why both its
/// dice start where nihil's bare ones do and climb by eating rather than by
/// shopping. What it gets instead is technique: the estoc's lunge
/// ([`Lunges`]), the rapier's rhythm ([`BuildsMomentum`], built on the
/// creature itself since there is no blade to build it on), a hunter's
/// quiet ([`Stealthy`]), and Bide — which it pays for like anyone else,
/// because a coiled spring is not something a wolf is born knowing.
fn wear_lurk(world: &mut World, player: Entity) {
    world.entity_mut(player).insert((
        Renderable {
            glyph: '@',
            color: Color::Magenta,
        },
        Fighter {
            hp: lurk::START_HP,
            max_hp: lurk::START_HP,
            power: lurk::START_POWER,
            max_power: lurk::START_POWER,
            power_bonus: 0,
            armor: lurk::START_ARMOR,
            armor_bonus: 0,
        },
        Magic {
            points: lurk::START_MAGIC,
            max_points: lurk::START_MAGIC,
        },
        Speed::new(SpeedKind::Quick),
    ));
    grant_all(world, player, LURK_GRANTS);
    if let Some(mut spellset) = world.get_mut::<Spellset>(player) {
        spellset.slots.push(SpellEffect::Bide);
    }
}

/// The tempo `player` returns to when a staircase lifts whatever the floor
/// lent them — their own, not everyone's [`SpeedKind::Normal`].
pub fn innate_tempo(world: &World, player: Entity) -> SpeedKind {
    if world.get::<Lurk>(player).is_some() {
        return SpeedKind::Quick;
    }
    world
        .get::<MonsterBody>(player)
        .map_or(SpeedKind::Normal, |b| b.0.speed)
}

/// Why `user` cannot put something on, or `None` if they can.
///
/// The one gate. A monster only ever wears what its own bestiary row rolled
/// for it, and every row that rolls gear is a row marked
/// [`crate::effects::ItemUser`] — so that marker already *is* the game's
/// answer to "can this species use equipment", and a player wearing the
/// species answers to it too. A lurk answers to its own shape: a ring goes on
/// a claw, and nothing else goes anywhere.
pub fn equip_refusal(world: &World, user: Entity, slot: Slot, item_name: &str) -> Option<String> {
    if world.get::<Lurk>(user).is_some() {
        return (slot != Slot::Finger).then(|| strings::no_hands_lurk(item_name));
    }
    let Some(MonsterBody(def)) = world.get::<MonsterBody>(user).copied() else {
        return None;
    };
    (world.get::<crate::effects::ItemUser>(user).is_none())
        .then(|| strings::no_hands_monster(def.display_name(), item_name))
}

/// One corpse, and a [`lurk::GROWTH_CHANCE`] roll on it. A hit grows the
/// lurk: +1 to one of the four numbers on the HUD, drawn at random.
///
/// Rolled per corpse rather than counted toward a tenth one, which is worth
/// the paragraph: a counter is a thing to pace yourself against and a thing
/// the save file would have to remember, and neither is what eating is. The
/// odds carry no state at all, so there is nothing to lose on a reload.
///
/// It never asks whose claws it was, for the same reason
/// [`crate::score::award_kill`] doesn't: half the ways a monster dies down
/// here have no killer to ask about, and a wolf that only grew on kills of
/// the approved sort would be teaching the player how to hunt.
pub fn feed(world: &mut World) {
    let Some(player) = world
        .query_filtered::<Entity, (With<Player>, With<Lurk>)>()
        .iter(world)
        .next()
    else {
        return;
    };
    if !world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(lurk::GROWTH_CHANCE)
    {
        return;
    }
    let grown = match world.resource_mut::<GameRng>().0.gen_range(0..4) {
        0 => {
            let mut f = world.get_mut::<Fighter>(player).unwrap();
            f.max_hp += lurk::GROWTH_STEP;
            f.hp += lurk::GROWTH_STEP;
            strings::hp_abbr()
        }
        1 => {
            let mut m = world.get_mut::<Magic>(player).unwrap();
            m.max_points = m.max_points.saturating_add(lurk::GROWTH_STEP as u8);
            m.points = m.points.saturating_add(lurk::GROWTH_STEP as u8);
            strings::magic_abbr()
        }
        2 => {
            let mut f = world.get_mut::<Fighter>(player).unwrap();
            f.max_power += lurk::GROWTH_STEP;
            f.power += lurk::GROWTH_STEP;
            strings::power_abbr()
        }
        _ => {
            let mut f = world.get_mut::<Fighter>(player).unwrap();
            f.armor += lurk::GROWTH_STEP;
            strings::armor_abbr()
        }
    };
    world
        .resource_mut::<GameLog>()
        .add(strings::fear_the_lurk(lurk::GROWTH_STEP, grown));
}

// ---------------------------------------------------------------------------
// Playing as a monster
// ---------------------------------------------------------------------------

/// The bestiary row the player is wearing — `-am <species>`, and nothing
/// else in the game puts it on. Everything that has to ask "what is this
/// creature really" asks this rather than reading the glyph back out of
/// [`Renderable`] or the species back out of [`Name`]: whether a body has
/// hands enough to equip anything ([`crate::equipment::toggle_equipped`]),
/// and what tempo it returns to when a staircase lifts the floor's own
/// ([`crate::conditions::clear_player_conditions`]).
#[derive(Component, Clone, Copy)]
pub struct MonsterBody(pub &'static MonsterDef);
