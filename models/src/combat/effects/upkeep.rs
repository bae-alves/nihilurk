//! Effects over time: `tick_effects` ages the ledger each turn, `hold` pins a creature in
//! place, `stone_chip` says how much of a blow a petrified creature takes, and
//! `turns_left` and `dangers_of` read the ledger back.

use super::*;

// ---------------------------------------------------------------------------
// The clock
// ---------------------------------------------------------------------------

/// Ages every effect held for a number of turns, and ends the ones that run
/// out.
///
/// One system for every timed effect there will ever be. It replaced
/// `snare_system`, which did this for exactly one component and could not have
/// done it for a second without being copied.
///
/// Runs at the very top of the turn, in the slot `snare_system` held, so a
/// creature's last turn of being held is spent held.
///
/// Assumes nothing: `smoke_system`, the only step before it, ages puffs and
/// touches no effect. It returns early once `Ending::player_dead` is set, so a
/// finished run stops ageing its ledgers.
pub fn tick_effects(world: &mut World) {
    if world
        .get_resource::<crate::state::Ending>()
        .is_some_and(|e| e.player_dead)
    {
        return;
    }

    let ticking: Vec<Entity> = world
        .query_filtered::<Entity, With<Effects>>()
        .iter(world)
        .collect();

    for entity in ticking {
        let Some(mut ledger) = world.get_mut::<Effects>(entity) else {
            continue;
        };
        let mut ended: Vec<&'static str> = Vec::new();
        for held in ledger.0.iter_mut() {
            let Lifetime::Turns(left) = held.lifetime else {
                continue;
            };
            let left = left.saturating_sub(1);
            held.lifetime = Lifetime::Turns(left);
            if left == 0 {
                ended.push(held.id);
            }
        }
        if ended.is_empty() {
            continue;
        }

        revoke_matching(world, entity, |h| h.lifetime == Lifetime::Turns(0));

        if world.get::<Player>(entity).is_none() {
            continue;
        }
        for id in ended {
            let Some(line) = Effect::by_id(id).and_then(|e| e.ends) else {
                continue;
            };
            world.resource_mut::<GameLog>().add(line.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// Stone
// ---------------------------------------------------------------------------

/// What a hit for `amount` actually does to a petrified creature, and the one
/// sentence it is reported with.
pub struct Chip {
    /// The HP that gets through the stone: a chip at most, and nothing at all
    /// against a creature already on its last point.
    pub through: i32,
    /// "You are chipped for 1 damage." — the line the blow is reported with,
    /// in place of whatever the source would otherwise have said.
    pub line: String,
}

/// The whole of what being [`Petrified`] does to a hit: at most
/// [`CHIP_DAMAGE`] gets through, and never the creature's last point of HP.
/// `None` when the target is not stone, which is the caller's signal to
/// resolve and report the hit its own way.
///
/// The one thing stone does not stop is a war hammer — see
/// `crate::combat`, which asks whether the attacker carries
/// [`ShattersStone`] before it asks this at all. A hit with no attacker behind
/// it (a ray, a flame, a falling dart) has nothing to ask.
///
/// It lives here, with the effect, because both damage paths have to obey it
/// and say it identically — [`crate::combat::resolve_attack`], which applies
/// its own HP, and [`crate::helpers::apply_hit`], which every other source of
/// harm in the game goes through. Two copies of this rule would be two
/// answers to "can a petrified player die".
pub fn stone_chip(world: &World, target: Entity, amount: i32) -> Option<Chip> {
    world.get::<Petrified>(target)?;
    let hp = world.get::<Fighter>(target).map_or(0, |f| f.hp);
    let through = amount.clamp(0, CHIP_DAMAGE).min((hp - 1).max(0));
    let (who, verb) = match world.get::<Player>(target).is_some() {
        true => ("You".to_string(), "are"),
        false => (
            format!("The {}", crate::helpers::item_label(world, target)),
            "is",
        ),
    };
    let line = match through {
        0 => format!("{who} {verb} chipped for no damage."),
        n => format!("{who} {verb} chipped for {n} damage."),
    };
    Some(Chip { through, line })
}

/// The four holds: what keeps a creature where it stands, each on its own
/// clock. They are listed once, here, because the two ways out of one — the
/// scroll of teleportation and the wand of teleport away — have to let go of
/// all four, and a fifth hold added to the registry and forgotten at one of
/// those two sites would strand a creature nowhere near what was holding it.
///
/// [`Petrified`] is a hold and is deliberately not here: it is not something
/// holding the victim in a place, it is the victim's own body, and a teleport
/// takes the statue with it.
pub const HOLDS: [Grant; 4] = [
    Grant::of::<Asleep>(),
    Grant::of::<Pinned>(),
    Grant::of::<Rooted>(),
    Grant::of::<Clamped>(),
];

/// Holds `victim` for `turns` more turns — the steel jaws of a bear trap, a
/// lungful of sleeping gas, the words of a scroll of hold monster.
///
/// Deliberately silent: a hold arrives from a trap, a scroll or a cloud of
/// gas, and the sentence the player reads belongs to whichever it was.
/// Returns whether it changed anything — a creature already held this long by
/// the same thing is left alone rather than having its sentence shortened.
pub fn hold(world: &mut World, victim: Entity, grant: Grant, turns: u32) -> bool {
    if turns == 0 {
        return false;
    }
    let Some(id) = grant.effect_id() else {
        return false;
    };
    let standing = world
        .get::<Effects>(victim)
        .map(|l| {
            l.0.iter()
                .filter(|h| h.id == id)
                .filter_map(|h| match h.lifetime {
                    Lifetime::Turns(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    if standing >= turns {
        return false;
    }
    // Replace rather than stack: a second dose of the same gas is a longer
    // sleep, not two sleeps running down side by side.
    revoke_matching(world, victim, |h| h.id == id);
    lend(world, victim, grant, Lifetime::Turns(turns))
}

/// How many turns `entity` has left of `grant`, or `None` if it is not held by
/// it at all.
pub fn turns_left(world: &World, entity: Entity, grant: Grant) -> Option<u32> {
    let id = grant.effect_id()?;
    world
        .get::<Effects>(entity)?
        .0
        .iter()
        .find_map(|h| match (h.id == id, h.lifetime) {
            (true, Lifetime::Turns(n)) => Some(n),
            _ => None,
        })
}

/// Every notable thing a creature carries, named plainly — what `Look` warns
/// about when the reticle lands on it.
///
/// Derived from [`EFFECTS`] rather than listed separately, so a new marker
/// cannot be left out of `Look`'s warnings.
///
/// A wielded launcher is the one entry that is not a marker — it is a question
/// about gear, not a property of the creature — so it is asked separately and
/// last.
pub fn dangers_of(world: &World, entity: Entity) -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = EFFECTS
        .iter()
        .filter(|e| e.grant.probe(world, entity))
        .filter_map(|e| e.beware)
        .collect();
    seen.dedup();
    if crate::equipment::wielded_launcher(world, entity).is_some() {
        seen.push(strings::beware_ranged_shots());
    }
    if world
        .get::<crate::components::Spellset>(entity)
        .is_some_and(|s| !s.slots.is_empty())
    {
        seen.push(strings::beware_spellcasting());
    }
    seen
}
