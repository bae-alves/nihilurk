//! What an effect does *on its own* — one table per moment it can act on.
//!
//! Some effects are answers to a question another system asks — [`FireImmune`]
//! only matters when a wand of fire goes off. Others act by themselves, and
//! there are two moments they can pick:
//!
//! * **Every turn** ([`PASSIVE_ABILITIES`]) — while you carry
//!   [`AggravatesMonsters`], the floor keeps noticing you.
//! * **On a blow that lands** ([`ON_HIT_ABILITIES`]) — a charmed pair of hands
//!   passes its confusion on, an aquator's touch eats the armour it hit.
//!
//! Both tables name their effect with the same [`Grant`] handle the bestiary and
//! the ring catalog use, so nothing here knows or cares what granted the
//! ability. A ring grants it today; a cursed blade or a monster's aura could
//! grant it tomorrow and the behaviour would follow, untouched. And both are the
//! answer to the same question: *when* does this fire, and *what* does it do.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::combat::resolve_attack;
use crate::components::{
    Backpack, ConfusingTouch, Curse, EntityMoved, ExtraMonsterRound, Fighter, GameLog, MagicWard,
    Mob, Player, Position, SnareKind, TrapEffect,
};
use crate::conditions::snare;
use crate::effects::{
    AggravatesMonsters, Batty, Binds, BuildsMomentum, Cleaves, Freezing, Gorgon, Grant, HeavySwing,
    Momentum, Regenerates, RustsArmor, SelfDamageOnHit, StealsAndFlees, StealsAndVanishes,
    SustainsStrength, Teleportitis, Vampiric, Venomous,
};
use crate::equipment::{Slot, equipped_in};
use crate::helpers::{adjacent_mobs, apply_damage, item_label};
use crate::map::GameRng;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
use crate::constants::monsters::{
    ICE_MONSTER_PARALYZE_CHANCE, RATTLESNAKE_POWER_DRAIN, VAMPIRE_MAX_HP_DRAIN,
};

/// One self-acting passive: the effect that arms it, the odds it fires on any
/// given turn, the mechanic it runs, and the line logged when it goes off.
pub struct PassiveAbility {
    /// The marker component that arms this ability.
    pub effect: Grant,
    /// Probability it fires on each turn its bearer acts.
    pub chance: f64,
    /// The mechanic, run on the bearer. Reports whether it actually did
    /// anything: a ring of regeneration wins its roll every other turn and must
    /// be silent on the ones where there was nothing left to mend.
    pub action: fn(&mut World, Entity) -> bool,
    /// Flavour logged when the mechanic did something. Written in second
    /// person, so it is only logged for the player.
    pub flavour: &'static str,
}

/// Every passive that acts on its own. A new "happens at random while you have
/// it" ability is one row plus the mechanic it calls.
pub const PASSIVE_ABILITIES: &[PassiveAbility] = &[
    PassiveAbility {
        effect: Grant::of::<AggravatesMonsters>(),
        chance: 0.10,
        action: crate::items::aggravate_all_monsters,
        flavour: "You yip! The whole floor turns your way.",
    },
    PassiveAbility {
        effect: Grant::of::<Regenerates>(),
        chance: 0.50,
        action: crate::items::regenerate,
        flavour: "The ring on your finger is warm.",
    },
    // Rogue's teleportitis, at NetHack's odds: 1 in 85 turns, and the jump lands
    // at the top of the bearer's next turn (see `passive_ability_system`).
    PassiveAbility {
        effect: Grant::of::<Teleportitis>(),
        chance: 1.0 / 85.0,
        action: crate::items::teleportitis,
        flavour: "Something on your finger is pleased with itself.",
    },
];

// ---------------------------------------------------------------------------
// On a blow that lands
// ---------------------------------------------------------------------------

/// One thing that happens *because a blow connected*: the effect the attacker
/// must carry, which blows count, and what it does to whoever was hit.
///
/// The two rows are the aquator's corrosive touch and a scroll of monster
/// confusion's charm. Neither is a special case in [`crate::combat`] any more —
/// `resolve_attack` fires the table and never learns what is in it.
pub struct OnHitAbility {
    /// The marker component on the *attacker* that arms this.
    pub effect: Grant,
    /// Whether a glancing scrape counts. Acid does not care that the armour
    /// turned the blow — it landed on the armour. A charm needs to reach skin.
    pub on_glancing: bool,
    /// Whether the killing blow counts. There is no point charming a corpse;
    /// there is no harm eating the plus off what it was wearing.
    pub on_lethal: bool,
    /// The mechanic, run as `(attacker, target)`.
    pub action: fn(&mut World, Entity, Entity),
}

/// Every on-hit ability in the game.
pub const ON_HIT_ABILITIES: &[OnHitAbility] = &[
    OnHitAbility {
        effect: Grant::of::<ConfusingTouch>(),
        on_glancing: false,
        on_lethal: false,
        action: crate::items::discharge_confusing_touch,
    },
    OnHitAbility {
        effect: Grant::of::<RustsArmor>(),
        on_glancing: true,
        on_lethal: true,
        action: corrode,
    },
    OnHitAbility {
        effect: Grant::of::<Batty>(),
        on_glancing: true,
        on_lethal: false,
        action: batty_hop,
    },
    OnHitAbility {
        effect: Grant::of::<Freezing>(),
        on_glancing: false,
        on_lethal: false,
        action: freezing_touch,
    },
    OnHitAbility {
        effect: Grant::of::<Venomous>(),
        on_glancing: false,
        on_lethal: false,
        action: venomous_bite,
    },
    OnHitAbility {
        effect: Grant::of::<Vampiric>(),
        on_glancing: false,
        on_lethal: false,
        action: vampiric_drain,
    },
    OnHitAbility {
        effect: Grant::of::<Binds>(),
        on_glancing: true,
        on_lethal: false,
        action: bind_victim,
    },
    OnHitAbility {
        effect: Grant::of::<StealsAndFlees>(),
        on_glancing: false,
        on_lethal: false,
        action: crate::items::leprechaun_theft,
    },
    OnHitAbility {
        effect: Grant::of::<StealsAndVanishes>(),
        on_glancing: false,
        on_lethal: false,
        action: crate::items::nymph_theft,
    },
    OnHitAbility {
        effect: Grant::of::<HeavySwing>(),
        on_glancing: false,
        on_lethal: false,
        action: heavy_stagger,
    },
    OnHitAbility {
        effect: Grant::of::<SelfDamageOnHit>(),
        on_glancing: true,
        on_lethal: true,
        action: chaos_recoil,
    },
    OnHitAbility {
        effect: Grant::of::<BuildsMomentum>(),
        on_glancing: false,
        on_lethal: true,
        action: build_momentum,
    },
];

/// The battle axe's cleave: everything else standing next to the wielder when
/// their swing lands takes the same swing, right along with the target
/// already struck. A no-op for anything not wielding one — the engine calls
/// this after every player attack rather than checking first.
pub fn cleave_attack(world: &mut World, attacker: Entity, already_hit: Entity) {
    if !is_player(world, attacker) || world.get::<Cleaves>(attacker).is_none() {
        return;
    }
    let Some(pos) = world.get::<Position>(attacker).copied() else {
        return;
    };
    for target in adjacent_mobs(world, pos, attacker) {
        if target == already_hit || world.get::<Fighter>(target).is_none() {
            continue;
        }
        resolve_attack(world, attacker, target);
    }
}

/// Every weapon trick in this file is the *player's* alone: a monster that
/// steals, catches or spawns wielding one of these still fights the plain way
/// — a normal swing, or a shot if what's in its hand is a launcher instead.
/// Each action below checks this first and does nothing at all for anything
/// else, the same one-line gate every time.
fn is_player(world: &World, entity: Entity) -> bool {
    world.get::<Player>(entity).is_some()
}

/// The greatclub's weight: a hit that lands staggers its victim outright —
/// one turn with no action at all, the same [`SnareKind::Sleep`] a sleep trap
/// uses — and the swing costs its wielder a beat of their own, spent as one
/// extra monster round the instant the turn schedule asks for it (see
/// `crate::ai::ai`). The player's trick alone — see [`is_player`].
fn heavy_stagger(world: &mut World, attacker: Entity, target: Entity) {
    if !is_player(world, attacker) {
        return;
    }
    let staggered = snare(world, target, SnareKind::Sleep, 1);
    if staggered {
        let line = match world.get::<Player>(target).is_some() {
            true => "The blow staggers you — you can't gather yourself to answer it!".to_string(),
            false => format!(
                "The {} reels from the blow, staggered!",
                item_label(world, target)
            ),
        };
        world.resource_mut::<GameLog>().add(line);
    }
    world.resource_mut::<ExtraMonsterRound>().0 = true;
}

/// The chaos blade's price: every hit that connects bites its wielder for a
/// point of their own HP — "the edge of chaos bites you." The player's trick
/// alone — see [`is_player`].
fn chaos_recoil(world: &mut World, attacker: Entity, _target: Entity) {
    if !is_player(world, attacker) {
        return;
    }
    apply_damage(world, attacker, 1);
    world
        .resource_mut::<GameLog>()
        .add("The edge of chaos bites you!".to_string());
}

/// The rapier's technique: every hit that lands adds two points to the
/// weapon's own [`Momentum`] — on top of, never overwriting, whatever
/// enchantment plus it already carries. Lifted the moment the weapon leaves
/// the wielder's hand (see [`crate::equipment::force_unequip`]) or the
/// wielder does anything but keep swinging it (see
/// [`crate::equipment::reset_momentum`]). The player's trick alone — see
/// [`is_player`].
fn build_momentum(world: &mut World, attacker: Entity, _target: Entity) {
    if !is_player(world, attacker) {
        return;
    }
    let Some(weapon) = equipped_in(world, attacker, Slot::Hand) else {
        return;
    };
    let built = world.get::<Momentum>(weapon).map_or(0, |m| m.0);
    world.entity_mut(weapon).insert(Momentum(built + 2));
}

/// [`crate::equipment::corrode_armor`] with the table's shape: an on-hit
/// ability is handed both ends of the blow, and this one only cares about the
/// end that was wearing something.
fn corrode(world: &mut World, _attacker: Entity, target: Entity) {
    crate::equipment::corrode_armor(world, target);
}

/// "Batty": every blow it lands, the attacker itself tries to hop to a random
/// adjacent tile right afterward — the bat's (and the phantom's) erratic
/// flitting. A no-op when nothing open is free to land on, and tags the
/// landing tile [`EntityMoved`] so a bat that hops onto a trap still springs
/// it.
fn batty_hop(world: &mut World, attacker: Entity, _target: Entity) {
    let Some(pos) = world.get::<Position>(attacker).copied() else {
        return;
    };
    let Some((x, y)) = crate::helpers::free_adjacent_tile(world, pos) else {
        return;
    };
    if let Some(mut p) = world.get_mut::<Position>(attacker) {
        p.x = x;
        p.y = y;
    }
    world.entity_mut(attacker).insert(EntityMoved);
}

/// The ice monster's freeze: [`ICE_MONSTER_PARALYZE_CHANCE`] on every clean
/// hit of locking the victim's limbs up outright — the same paralysis a
/// potion does.
fn freezing_touch(world: &mut World, _attacker: Entity, target: Entity) {
    if !world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(ICE_MONSTER_PARALYZE_CHANCE)
    {
        return;
    }
    crate::conditions::paralyse(world, target);
}

/// The rattlesnake's bite: [`RATTLESNAKE_POWER_DRAIN`] points of base power,
/// permanently — like the dart trap's poison, but with no floor of 1, so a
/// long enough fight can drive a victim's power negative. A ring of strength
/// ([`SustainsStrength`]) shrugs it off exactly as it does the trap.
fn venomous_bite(world: &mut World, _attacker: Entity, target: Entity) {
    if world.get::<SustainsStrength>(target).is_some() {
        if world.get::<Player>(target).is_some() {
            world
                .resource_mut::<GameLog>()
                .add("The venom burns, but your strength holds firm.".to_string());
        }
        return;
    }
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.power -= RATTLESNAKE_POWER_DRAIN;
    }
    if world.get::<Player>(target).is_some() {
        world
            .resource_mut::<GameLog>()
            .add("Venom courses through you — your strength ebbs away.".to_string());
    }
}

/// The vampire's touch: [`VAMPIRE_MAX_HP_DRAIN`] points off the victim's
/// *maximum* HP, permanently, clamping current HP down with it if it now
/// exceeds the new ceiling.
fn vampiric_drain(world: &mut World, _attacker: Entity, target: Entity) {
    let Some(mut fighter) = world.get_mut::<Fighter>(target) else {
        return;
    };
    fighter.max_hp = (fighter.max_hp - VAMPIRE_MAX_HP_DRAIN).max(1);
    if fighter.hp > fighter.max_hp {
        fighter.hp = fighter.max_hp;
    }
    if world.get::<Player>(target).is_some() {
        world
            .resource_mut::<GameLog>()
            .add("A deathly chill spreads through you — your vitality is drained!".to_string());
    }
}

/// The venus flytrap's (and a revealed xeroc's) bite: clamps the victim in a
/// bear trap's jaws — the same [`SnareKind::Bear`] snare, for the same number
/// of turns a bear trap holds for.
fn bind_victim(world: &mut World, attacker: Entity, target: Entity) {
    let turns = crate::traps::TrapDef::of(TrapEffect::Bear).snare_turns;
    if !snare(world, target, SnareKind::Bear, turns) {
        return;
    }
    let name = item_label(world, attacker);
    let line = match world.get::<Player>(target).is_some() {
        true => format!(
            "The {name} clamps its jaws around your leg — you can't take a step, but your arms are free!"
        ),
        false => format!(
            "The {name} clamps its jaws around the {}!",
            item_label(world, target)
        ),
    };
    world.resource_mut::<GameLog>().add(line);
}

// ---------------------------------------------------------------------------
// The medusa's gaze
// ---------------------------------------------------------------------------

/// The medusa's gaze: petrify the player outright the instant they attack,
/// fire at, or zap the creature — mechanically identical to
/// [`SnareKind::Sleep`] (the same snare, the same [`SLEEP_TURNS`]), only the
/// flavour is stone rather than slumber. Certain, not a roll — looking upon a
/// medusa is the whole danger — and it lands whether or not the blow itself
/// does; the gaze doesn't wait to see if you missed.
///
/// A no-op for anything that isn't the player looking upon a [`Gorgon`]: a
/// medusa's own kind is unmoved by each other, and nothing but a person's eyes
/// can be turned to stone by this.
pub(crate) fn medusa_gaze(world: &mut World, looker: Entity, seen: Entity) {
    if world.get::<Player>(looker).is_none() || world.get::<Gorgon>(seen).is_none() {
        return;
    }
    let turns = crate::constants::scrolls::SLEEP_TURNS;
    if !snare(world, looker, SnareKind::Sleep, turns) {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add("Your eyes meet the medusa's — and your flesh turns to cold stone!".to_string());
}

// ---------------------------------------------------------------------------
// Theft: the leprechaun and the nymph
// ---------------------------------------------------------------------------

/// A uniformly random item in `victim`'s pack that isn't currently equipped —
/// the Element of Yoord excepted, since nothing in the dungeon can lift that
/// off you. `None` for an empty pack, or one holding nothing but the relic.
pub(crate) fn steal_unequipped_item(world: &mut World, victim: Entity) -> Option<Entity> {
    let items = world.get::<Backpack>(victim)?.items.clone();
    let stealable: Vec<Entity> = items
        .into_iter()
        .filter(|&e| world.get::<crate::components::Amulet>(e).is_none())
        .collect();
    if stealable.is_empty() {
        return None;
    }
    let idx = world
        .resource_mut::<GameRng>()
        .0
        .gen_range(0..stealable.len());
    let item = stealable[idx];
    if let Some(mut bp) = world.get_mut::<Backpack>(victim) {
        bp.items.retain(|&e| e != item);
    }
    Some(item)
}

/// A uniformly random piece of gear `victim` currently has equipped that
/// isn't cursed onto them — a curse holds even against a nymph's fingers.
/// `None` if there is nothing to take.
pub(crate) fn steal_equipped_item(world: &mut World, victim: Entity) -> Option<Entity> {
    let stealable: Vec<Entity> = crate::equipment::equipped_items(world, victim)
        .into_iter()
        .filter(|&e| world.get::<Curse>(e).is_none())
        .collect();
    if stealable.is_empty() {
        return None;
    }
    let idx = world
        .resource_mut::<GameRng>()
        .0
        .gen_range(0..stealable.len());
    let item = stealable[idx];
    crate::equipment::force_unequip(world, item);
    crate::equipment::sync_equipment_effects(world, victim);
    if let Some(mut bp) = world.get_mut::<Backpack>(victim) {
        bp.items.retain(|&e| e != item);
    }
    Some(item)
}

/// Fires every on-hit ability `attacker` has armed against `target`. Called by
/// [`crate::combat::resolve_attack`] for every blow that drew blood, with the
/// shape of the blow so each row can bow out of the ones it does not want.
pub fn fire_on_hit(world: &mut World, attacker: Entity, target: Entity, blow: Blow) {
    // The move Magic Ward: nothing a blow carries with it — a rattlesnake's
    // drain, a vampire's kiss, an aquator's rust — reaches whoever is
    // wearing one, for the rest of the floor. The damage itself already
    // landed; this is only the trick riding on top of it.
    if world.get::<MagicWard>(target).is_some() {
        return;
    }
    for ability in ON_HIT_ABILITIES {
        if blow.glancing && !ability.on_glancing {
            continue;
        }
        if blow.lethal && !ability.on_lethal {
            continue;
        }
        if !ability.effect.probe(world, attacker) {
            continue;
        }
        (ability.action)(world, attacker, target);
    }
}

/// What kind of blow just landed, for the rows that care. Damage above zero is
/// assumed — a blow that did nothing never reaches the table.
#[derive(Clone, Copy)]
pub struct Blow {
    /// The armour ate it and the player's chip-damage floor is all that got
    /// through.
    pub glancing: bool,
    /// It was the last one.
    pub lethal: bool,
}

/// Rolls every passive ability its bearer currently has armed.
///
/// Registered at the **tail** of the turn schedule, after the monsters have
/// moved and before visibility is recomputed. The schedule only runs on turns
/// the player took an action, so "every turn" means "every action" — and
/// running last means a passive that *moves* its bearer lands at the top of
/// their next turn: they see where they ended up and act from there before
/// anything on the floor gets another move. That is the difference between a
/// ring of teleportation and a curse.
///
/// The system asks only "who carries this effect" — it never asks where the
/// effect came from, which is the whole point.
pub fn passive_ability_system(world: &mut World) {
    // Only creatures. An effect never lands on an item: a ring carries
    // `Grants`, and it is the *wearer* who ends up with `Regenerates` on them
    // (see `crate::equipment::sync_equipment_effects`). Asking the whole world
    // would walk every scroll and every wall-bound arrow to find that out.
    let actors: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Player>, With<Mob>)>>()
        .iter(world)
        .collect();

    for ability in PASSIVE_ABILITIES {
        let bearers: Vec<Entity> = actors
            .iter()
            .copied()
            .filter(|&e| ability.effect.probe(world, e))
            .collect();

        for bearer in bearers {
            if !world.resource_mut::<GameRng>().0.gen_bool(ability.chance) {
                continue;
            }
            let did_something = (ability.action)(world, bearer);
            if did_something && world.get::<Player>(bearer).is_some() {
                world
                    .resource_mut::<GameLog>()
                    .add(ability.flavour.to_string());
            }
        }
    }
}
