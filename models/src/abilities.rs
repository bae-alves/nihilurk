//! What an effect does *on its own* — one table, one row per ability.
//!
//! Some effects are answers to a question another system asks — [`FireImmune`]
//! only matters when a wand of fire goes off. Others act by themselves, and
//! [`ABILITIES`] is all of those: each row names the effect that arms it, the
//! [`Moment`] it fires at, and what it does.
//!
//! **The moment is a field, not a table.** That is the whole shape of this
//! file. It used to be a table per moment — one for "every turn", one for "on
//! a blow that lands" — which meant a moment nobody had written a table for
//! was an `if` welded into whichever system happened to be standing there.
//! Six of the bestiary's markers reached no table at all for that reason: the
//! medusa's gaze was hand-called from four sites, the slime's split was a
//! hardcoded line in `took_damage`, and adding a fifth path meant remembering
//! to call it again.
//!
//! Adding a moment is a [`Moment`] variant, one arm in [`fires_at`], and an
//! entry point that says who the bearer is. Adding an ability at a moment that
//! already exists is one row.
//!
//! Every row names its effect with the same [`Grant`] handle the bestiary and
//! the ring catalog use, so nothing here knows or cares what granted the
//! ability. A ring grants it today; a cursed blade or a monster's aura could
//! grant it tomorrow and the behaviour would follow, untouched.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::combat::resolve_attack;
use crate::components::{
    Backpack, Curse, EntityMoved, ExtraMonsterRound, Fighter, GameLog, Mob, Player, Position,
    SpellEffect, TrapEffect,
};
use crate::conditions::snare;
use crate::effects::{
    AggravatesMonsters, Asleep, Batty, Binds, BuildsMomentum, Cleaves, ConfusingTouch, FireBreath,
    Freezing, Gorgon, Grant, HeavySwing, LightningBreath, MagicWard, Momentum, Petrified, Pinned,
    Regenerates, RustsArmor, SelfDamageOnHit, Splits, StealsAndFlees, StealsAndVanishes,
    Teleportitis, Vampiric, Venomous,
};
use crate::equipment::{Slot, equipped_in};
use crate::helpers::{adjacent_mobs, apply_damage, item_label};
use crate::map::GameRng;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
use crate::constants::monsters::{DRAGON_FIREBALL_CHANCE, EEL_LIGHTNING_CHANCE};
use crate::constants::monsters::{
    ICE_MONSTER_PARALYZE_CHANCE, RATTLESNAKE_POWER_DRAIN, VAMPIRE_MAX_HP_DRAIN,
};

/// *When* an ability fires. One variant per moment the game has.
///
/// This is the field that replaced two tables with one. Before it, a moment
/// was a table — and a moment nobody had written a table for was an `if`
/// welded into whichever system happened to be standing there. Six of the
/// bestiary's markers reached no table at all for exactly that reason.
///
/// Adding a seventh moment is a variant here, one arm in the driver that
/// fires it, and nothing else. Adding an ability *at* an existing moment is
/// one row.
#[derive(Clone, Copy, PartialEq)]
pub enum Moment {
    /// A blow the bearer landed. The two flags are the only gating there is:
    /// does a glancing scrape count (acid says yes, a charm that needs skin
    /// says no), and does the killing blow count (there is no point charming
    /// a corpse).
    OnHit { glancing: bool, lethal: bool },
    /// Every turn its bearer acts, at these odds.
    EachTurn(f64),
    /// The bearer was hurt and lived. Fires from `helpers::took_damage`, which
    /// every damage path in the game already runs through.
    OnDamaged,
    /// The player turned their attention on the bearer — attacked, zapped or
    /// threw something at them. Fires before the blow itself, and whether or
    /// not it lands: looking upon a medusa is the danger.
    OnTargeted,
    /// The bearer is about to swing at something and would rather not, at
    /// these odds. A dragon breathes fire instead of clawing.
    ///
    /// The one moment that is a *decision* rather than a reaction: nothing has
    /// happened yet, and the row is bidding for the turn. A row that fires
    /// spends the turn and the blow never happens.
    InsteadOfAttacking(f64),
}

/// One ability: what arms it, when it fires, and what it does.
///
/// The `effect` is a [`Grant`] out of `EFFECTS`, so the ability never learns
/// what granted it — a ring today, a cursed blade tomorrow, and the behaviour
/// follows untouched. What the player is *warned* about is not here: that is
/// the `beware` field on the effect's own row, because it is true of the
/// marker whether or not the marker arms an ability.
pub struct Ability {
    /// The marker that arms this. On the **attacker** for [`Moment::OnHit`],
    /// on the bearer for everything else.
    pub effect: Grant,
    pub when: Moment,
    /// The player's alone. Every weapon trick in this file is: a monster that
    /// steals, catches or spawns wielding an estoc still fights the plain way.
    ///
    /// A field rather than a line in four different bodies, which is what it
    /// was — and a gate written four times is a gate that can be forgotten
    /// the fifth.
    pub player_only: bool,
    /// The mechanic. `target` is the creature on the other end for the
    /// moments that have one, and `None` where there isn't.
    ///
    /// Reports whether it did anything. Only [`Moment::EachTurn`] reads the
    /// answer — a ring of regeneration wins its roll every other turn and must
    /// be silent on the ones where there was nothing left to mend — but every
    /// row returns it, so a moment that grows a use for it later needs no new
    /// signature.
    pub action: fn(&mut World, Entity, Option<Entity>) -> bool,
    /// Logged when the mechanic did something, for the moments that speak.
    /// Written in the second person, so only the player is told.
    pub flavour: Option<&'static str>,
}

impl Ability {
    /// Whether this row should fire for `bearer` right now: they carry the
    /// marker, and they are allowed to use it.
    ///
    /// Public because it is the whole of "does this ability apply", and
    /// callers outside the drivers do ask — the look reticle, to decide
    /// whether pointing the cursor is safe.
    pub fn armed(&self, world: &World, bearer: Entity) -> bool {
        if self.player_only && world.get::<Player>(bearer).is_none() {
            return false;
        }
        self.effect.probe(world, bearer)
    }
}

/// Every ability in the game, at every moment it can fire.
///
/// One list. It was two — `PASSIVE_ABILITIES` and `ON_HIT_ABILITIES` — plus
/// six markers that had no list at all and were welded into `ai`, `combat`
/// and `took_damage` instead.
pub const ABILITIES: &[Ability] = &[
    // --- on a blow that lands -------------------------------------------
    row(
        Grant::of::<ConfusingTouch>(),
        hit(false, false),
        |w, a, t| {
            t.is_some_and(|t| {
                crate::items::discharge_confusing_touch(w, a, t);
                true
            })
        },
    ),
    row(Grant::of::<RustsArmor>(), hit(true, true), corrode),
    row(Grant::of::<Batty>(), hit(true, false), batty_hop),
    row(Grant::of::<Freezing>(), hit(false, false), freezing_touch),
    row(Grant::of::<Venomous>(), hit(false, false), venomous_bite),
    row(Grant::of::<Vampiric>(), hit(false, false), vampiric_drain),
    row(Grant::of::<Binds>(), hit(true, false), bind_victim),
    row(
        Grant::of::<StealsAndFlees>(),
        hit(false, false),
        |w, a, t| {
            t.is_some_and(|t| {
                crate::items::leprechaun_theft(w, a, t);
                true
            })
        },
    ),
    row(
        Grant::of::<StealsAndVanishes>(),
        hit(false, false),
        |w, a, t| {
            t.is_some_and(|t| {
                crate::items::nymph_theft(w, a, t);
                true
            })
        },
    ),
    mine(Grant::of::<HeavySwing>(), hit(false, false), heavy_stagger),
    mine(
        Grant::of::<SelfDamageOnHit>(),
        hit(true, true),
        chaos_recoil,
    ),
    mine(
        Grant::of::<BuildsMomentum>(),
        hit(false, true),
        build_momentum,
    ),
    // --- every turn ------------------------------------------------------
    Ability {
        effect: Grant::of::<AggravatesMonsters>(),
        when: Moment::EachTurn(0.10),
        player_only: false,
        action: |w, e, _| crate::items::aggravate_all_monsters(w, e),
        flavour: Some(strings::flavour_aggravates()),
    },
    Ability {
        effect: Grant::of::<Regenerates>(),
        when: Moment::EachTurn(0.50),
        player_only: false,
        action: |w, e, _| crate::items::regenerate(w, e),
        flavour: Some(strings::flavour_regenerates()),
    },
    // Rogue's teleportitis, at NetHack's odds: 1 in 85 turns, and the jump
    // lands at the top of the bearer's next turn (see `ability_system`).
    Ability {
        effect: Grant::of::<Teleportitis>(),
        when: Moment::EachTurn(1.0 / 85.0),
        player_only: false,
        action: |w, e, _| crate::items::teleportitis(w, e),
        flavour: Some(strings::flavour_teleportitis()),
    },
    // --- hurt and lived --------------------------------------------------
    // The slime's split. It used to be a hardcoded line in
    // `helpers::took_damage`, beside two things that are not abilities.
    row(Grant::of::<Splits>(), Moment::OnDamaged, |w, e, _| {
        crate::monsters::maybe_split(w, e);
        true
    }),
    // --- instead of the blow ---------------------------------------------
    // The dragon's fireball. It used to be an `if` in `ai::step_one_mob` —
    // a probe, a dice roll and a two-armed `match` sitting in the pathing
    // code, which is how one ability came to span five files with nothing
    // naming it. This row names it.
    //
    // What it fires is the spell, not a private copy of one: see
    // [`INNATE_SPELLS`].
    Ability {
        effect: Grant::of::<FireBreath>(),
        when: Moment::InsteadOfAttacking(DRAGON_FIREBALL_CHANCE),
        player_only: false,
        action: |w, mob, target| {
            target
                .and_then(|t| w.get::<Position>(t).copied())
                .is_some_and(|at| {
                    crate::items::apply_spell_effect(w, mob, at, SpellEffect::DragonBreath, 1);
                    true
                })
        },
        flavour: None,
    },
    // The eel's lightning: the same bid, casting the Thunderbolt.
    Ability {
        effect: Grant::of::<LightningBreath>(),
        when: Moment::InsteadOfAttacking(EEL_LIGHTNING_CHANCE),
        player_only: false,
        action: |w, mob, target| {
            target
                .and_then(|t| w.get::<Position>(t).copied())
                .is_some_and(|at| {
                    crate::items::apply_spell_effect(w, mob, at, SpellEffect::Thunderbolt, 1);
                    true
                })
        },
        flavour: None,
    },
    // --- looked upon -----------------------------------------------------
    // The medusa's gaze. It used to be hand-called from four sites, and a
    // fifth attack path would silently have missed it.
    row(
        Grant::of::<Gorgon>(),
        Moment::OnTargeted,
        |w, seen, looker| looker.is_some_and(|looker| medusa_gaze(w, looker, seen)),
    ),
];

/// The spell a born-with grant *is*.
///
/// A dragon's breath is the catalog's `Fireball` whether a dragon breathes it
/// at the player or a dragon-bodied player casts it at a dragon — one
/// mechanic, one row in [`crate::catalog::SPELLS`], two ways in. This table
/// is the pairing, and it buys three things at once: the ability row above
/// casts the spell rather than keeping a second copy of the blast,
/// [`crate::monsters::wear_monster`] puts it in the spell bar of a player born
/// with the grant, and [`crate::items::spell_cost`] charges nothing for it.
///
/// Zero, because the grant *is* the licence: a monster has no [`Magic`] to
/// spend and never did, so innate magic that costs magic points would simply
/// never fire.
pub const INNATE_SPELLS: &[(Grant, SpellEffect)] = &[
    (Grant::of::<FireBreath>(), SpellEffect::DragonBreath),
    (Grant::of::<LightningBreath>(), SpellEffect::Thunderbolt),
];

/// Whether `caster` carries the grant that makes `effect` innate to them —
/// asked by [`crate::items::spell_cost`], which makes it free, and by
/// [`crate::monsters::wear_monster`], which hands it over.
pub fn casts_innately(world: &World, caster: Entity, effect: SpellEffect) -> bool {
    INNATE_SPELLS
        .iter()
        .any(|(grant, spell)| *spell == effect && grant.probe(world, caster))
}

/// A row anything can carry.
const fn row(
    effect: Grant,
    when: Moment,
    action: fn(&mut World, Entity, Option<Entity>) -> bool,
) -> Ability {
    Ability {
        effect,
        when,
        player_only: false,
        action,
        flavour: None,
    }
}

/// A row that is the player's alone — see [`Ability::player_only`].
const fn mine(
    effect: Grant,
    when: Moment,
    action: fn(&mut World, Entity, Option<Entity>) -> bool,
) -> Ability {
    Ability {
        effect,
        when,
        player_only: true,
        action,
        flavour: None,
    }
}

/// Shorthand for the commonest moment.
const fn hit(glancing: bool, lethal: bool) -> Moment {
    Moment::OnHit { glancing, lethal }
}

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
/// one turn with no action at all, the same [`crate::effects::Asleep`] a sleep trap
/// uses — and the swing costs its wielder a beat of their own, spent as one
/// extra monster round the instant the turn schedule asks for it (see
/// `crate::ai::ai`). The player's trick alone — see [`is_player`].
fn heavy_stagger(world: &mut World, attacker: Entity, target: Option<Entity>) -> bool {
    let Some(target) = target else {
        return false;
    };
    let _ = attacker;
    let staggered = snare(world, target, Grant::of::<Asleep>(), 1);
    if staggered {
        let line = match world.get::<Player>(target).is_some() {
            true => strings::heavy_stagger_player().to_string(),
            false => strings::heavy_stagger_mob(&item_label(world, target)),
        };
        world.resource_mut::<GameLog>().add(line);
    }
    world.resource_mut::<ExtraMonsterRound>().0 = true;
    true
}

/// The chaos blade's price: every hit that connects bites its wielder for a
/// point of their own HP — "the edge of chaos bites you." The player's trick
/// alone — see [`is_player`].
fn chaos_recoil(world: &mut World, attacker: Entity, _target: Option<Entity>) -> bool {
    apply_damage(world, attacker, 1);
    world.resource_mut::<GameLog>().add(strings::chaos_recoil());
    true
}

/// The rapier's technique: every hit that lands adds two points to the
/// weapon's own [`Momentum`] — on top of, never overwriting, whatever
/// enchantment plus it already carries. Lifted the moment the weapon leaves
/// the wielder's hand (see [`crate::equipment::force_unequip`]) or the
/// wielder does anything but keep swinging it (see
/// [`crate::equipment::reset_momentum`]). The player's trick alone — see
/// [`is_player`].
fn build_momentum(world: &mut World, attacker: Entity, _target: Option<Entity>) -> bool {
    // On the steel if there is steel, on the fencer otherwise. A rapier keeps
    // its own build-up so that swapping blades mid-fight puts down what the
    // first one had going; a lurk has nothing to put down, and
    // [`crate::effects::loadout`] folds a modifier held by the creature
    // itself the same way it folds one held by its gear.
    let holder = equipped_in(world, attacker, Slot::Hand).unwrap_or(attacker);
    let built = world.get::<Momentum>(holder).map_or(0, |m| m.0);
    world.entity_mut(holder).insert(Momentum(built + 2));
    true
}

/// [`crate::equipment::corrode_armor`] with the table's shape: an on-hit
/// ability is handed both ends of the blow, and this one only cares about the
/// end that was wearing something.
fn corrode(world: &mut World, _attacker: Entity, target: Option<Entity>) -> bool {
    target.is_some_and(|t| crate::equipment::corrode_armor(world, t))
}

/// "Batty": every blow it lands, the attacker itself tries to hop to a random
/// adjacent tile right afterward — the bat's (and the phantom's) erratic
/// flitting. A no-op when nothing open is free to land on, and tags the
/// landing tile [`EntityMoved`] so a bat that hops onto a trap still springs
/// it.
fn batty_hop(world: &mut World, attacker: Entity, _target: Option<Entity>) -> bool {
    let Some(pos) = world.get::<Position>(attacker).copied() else {
        return false;
    };
    let Some((x, y)) = crate::helpers::free_adjacent_tile(world, pos) else {
        return false;
    };
    if let Some(mut p) = world.get_mut::<Position>(attacker) {
        p.x = x;
        p.y = y;
    }
    world.entity_mut(attacker).insert(EntityMoved);
    true
}

/// The ice monster's freeze: [`ICE_MONSTER_PARALYZE_CHANCE`] on every clean
/// hit of locking the victim's limbs up outright — the same paralysis a
/// potion does.
fn freezing_touch(world: &mut World, _attacker: Entity, target: Option<Entity>) -> bool {
    let Some(target) = target else {
        return false;
    };
    if !world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(ICE_MONSTER_PARALYZE_CHANCE)
    {
        return false;
    }
    crate::conditions::paralyse(world, target);
    true
}

/// The rattlesnake's bite: [`RATTLESNAKE_POWER_DRAIN`] points of base power,
/// permanently — like the dart trap's poison, but with no floor of 1, so a
/// long enough fight can drive a victim's power negative. A ring of strength
/// ([`SustainsStrength`]) shrugs it off exactly as it does the trap.
///
/// Announced either way, the same split `stagger`/`blind` already use: the
/// player reads it in the second person, anything else gets its own name in
/// the third. A bite the *player* lands (`-am rattlesnake`, or a hand-written
/// body that borrows the marker) used to drain in total silence — nothing
/// else in the ability table stays quiet just because the victim isn't you.
fn venomous_bite(world: &mut World, _attacker: Entity, target: Option<Entity>) -> bool {
    let Some(target) = target else {
        return false;
    };
    // No floor: a long enough fight with a rattlesnake drives a victim's
    // power negative, which is the bite's whole reputation.
    let drained = crate::conditions::drain_power(world, target, RATTLESNAKE_POWER_DRAIN, None);
    let is_player = world.get::<Player>(target).is_some();
    let took = matches!(drained, crate::conditions::Drain::Took);
    let name = item_label(world, target);
    let line = match (drained, is_player) {
        (crate::conditions::Drain::Nothing, _) => return false,
        (crate::conditions::Drain::Resisted, true) => strings::venom_resisted_player().to_string(),
        (crate::conditions::Drain::Resisted, false) => strings::venom_resisted_mob(&name),
        (crate::conditions::Drain::Took, true) => strings::venom_took_player().to_string(),
        (crate::conditions::Drain::Took, false) => strings::venom_took_mob(&name),
    };
    world.resource_mut::<GameLog>().add(line);
    took
}

/// The vampire's touch: [`VAMPIRE_MAX_HP_DRAIN`] points off the victim's
/// *maximum* HP, permanently, clamping current HP down with it if it now
/// exceeds the new ceiling.
///
/// Announced either way — see [`venomous_bite`], the same fix for the same
/// reason: a landed drain the player caused instead of suffered used to
/// leave nothing in the log to show for it.
fn vampiric_drain(world: &mut World, _attacker: Entity, target: Option<Entity>) -> bool {
    let Some(target) = target else {
        return false;
    };
    let Some(mut fighter) = world.get_mut::<Fighter>(target) else {
        return false;
    };
    fighter.max_hp = (fighter.max_hp - VAMPIRE_MAX_HP_DRAIN).max(1);
    if fighter.hp > fighter.max_hp {
        fighter.hp = fighter.max_hp;
    }
    let line = if world.get::<Player>(target).is_some() {
        strings::vampiric_drain_player().to_string()
    } else {
        strings::vampiric_drain_mob(&item_label(world, target))
    };
    world.resource_mut::<GameLog>().add(line);
    true
}

/// The venus flytrap's (and a revealed xeroc's) bite: clamps the victim in a
/// bear trap's jaws — the same [`crate::effects::Pinned`] snare, for the same number
/// of turns a bear trap holds for.
fn bind_victim(world: &mut World, attacker: Entity, target: Option<Entity>) -> bool {
    let Some(target) = target else {
        return false;
    };
    let turns = crate::traps::TrapDef::of(TrapEffect::Bear).snare_turns;
    if !snare(world, target, Grant::of::<Pinned>(), turns) {
        return false;
    }
    let name = item_label(world, attacker);
    let line = match world.get::<Player>(target).is_some() {
        true => strings::bind_victim_player(&name),
        false => strings::bind_victim_mob(&name, &item_label(world, target)),
    };
    world.resource_mut::<GameLog>().add(line);
    true
}

// ---------------------------------------------------------------------------
// The medusa's gaze
// ---------------------------------------------------------------------------

/// The medusa's gaze: petrify the player outright the instant they attack,
/// fire at, or zap the creature. Certain, not a roll — looking upon a medusa
/// is the whole danger — and it lands whether or not the blow itself does; the
/// gaze doesn't wait to see if you missed.
///
/// [`crate::effects::Petrified`] costs the player their turns exactly as sleep
/// does, and protects them while it lasts: nothing in the dungeon gets more
/// than a chip through stone, and nothing but a war hammer takes their last
/// point. It used to *be* sleep, which is why a player turned to stone was
/// told they had shaken off their drowsiness when it let go.
///
/// A no-op for anything that isn't the player looking upon a [`Gorgon`]: a
/// medusa's own kind is unmoved by each other, and nothing but a person's eyes
/// can be turned to stone by this.
fn medusa_gaze(world: &mut World, looker: Entity, _seen: Entity) -> bool {
    // `seen` carrying `Gorgon` is what armed the row; the gaze only works on a
    // person's eyes, which is the half the table cannot express.
    if world.get::<Player>(looker).is_none() {
        return false;
    }
    let turns = crate::constants::monsters::PETRIFY_TURNS;
    if !snare(world, looker, Grant::of::<Petrified>(), turns) {
        return false;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::medusa_gaze_line());
    true
}

// ---------------------------------------------------------------------------
// Theft: the leprechaun and the nymph
// ---------------------------------------------------------------------------

/// What a fleeing thief's hand closes on: a uniformly random item loose in
/// `victim`'s pack — nothing they have on, since worn gear sits in the pack
/// too — which leaves the pack with it. The Element of Yoord is in the draw
/// but never leaves: it comes back as the pick, still in the pack, for
/// [`crate::items::leprechaun_theft`] to answer. `None` for a pack with
/// nothing loose in it.
pub(crate) fn steal_unequipped_item(world: &mut World, victim: Entity) -> Option<Entity> {
    let loose: Vec<Entity> = world
        .get::<Backpack>(victim)?
        .items
        .iter()
        .copied()
        .filter(|&e| {
            world
                .get::<crate::equipment::Equipped>(e)
                .is_none_or(|eq| eq.by != Some(victim))
        })
        .collect();
    if loose.is_empty() {
        return None;
    }
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..loose.len());
    let item = loose[idx];
    if world.get::<crate::components::Amulet>(item).is_none()
        && let Some(mut bp) = world.get_mut::<Backpack>(victim)
    {
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
    // The spell Magic Ward: nothing a blow carries with it — a rattlesnake's
    // drain, a vampire's kiss, an aquator's rust — reaches whoever is
    // wearing one, for the rest of the floor. The damage itself already
    // landed; this is only the trick riding on top of it.
    //
    // The last ward check outside `helpers::apply_hit`, and deliberately so:
    // that one decides whether *damage* lands, and this decides whether the
    // rider does. They are the same word for two questions, and a blow that
    // hurt a warded creature can still be forbidden from poisoning them.
    if world.get::<MagicWard>(target).is_some() {
        return;
    }
    fire(
        world,
        Moment::OnHit {
            glancing: blow.glancing,
            lethal: blow.lethal,
        },
        attacker,
        Some(target),
    );
}

/// Fires every ability armed on `bearer` for one moment.
///
/// The one driver. Each moment's own entry point below decides *who* the
/// bearer is and *when* to call this; the matching, the player-only gate and
/// the flavour line are all here, once.
fn fire(world: &mut World, moment: Moment, bearer: Entity, other: Option<Entity>) -> bool {
    let mut anything = false;
    for ability in ABILITIES {
        if !fires_at(ability.when, moment) {
            continue;
        }
        if !ability.armed(world, bearer) {
            continue;
        }
        let did_something = (ability.action)(world, bearer, other);
        anything |= did_something;
        let Some(flavour) = ability.flavour else {
            continue;
        };
        if did_something && world.get::<Player>(bearer).is_some() {
            world.resource_mut::<GameLog>().add(flavour.to_string());
        }
    }
    anything
}

/// Whether a row written for `row` should fire at `now`.
///
/// Every moment but one is a plain match. [`Moment::OnHit`] is not, because
/// the row's two flags are not *which* blow it wants but which blows it will
/// tolerate: a row that says `glancing: false` is saying "not on a scrape",
/// and the blow that actually landed is what is being tested against it.
fn fires_at(row: Moment, now: Moment) -> bool {
    match (row, now) {
        (
            Moment::OnHit {
                glancing: takes_glancing,
                lethal: takes_lethal,
            },
            Moment::OnHit { glancing, lethal },
        ) => (takes_glancing || !glancing) && (takes_lethal || !lethal),
        (Moment::EachTurn(_), Moment::EachTurn(_)) => true,
        (Moment::InsteadOfAttacking(_), Moment::InsteadOfAttacking(_)) => true,
        (a, b) => a == b,
    }
}

/// Every creature the abilities can land on. An effect never lands on an item:
/// a ring carries `Grants`, and it is the *wearer* who ends up with
/// `Regenerates` on them (see `crate::equipment::sync_equipment_effects`).
fn actors(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<Entity, Or<(With<Player>, With<Mob>)>>()
        .iter(world)
        .collect()
}

/// The bearer was hurt and lived. Called from [`crate::helpers::took_damage`],
/// which every damage path in the game already runs through.
pub fn fire_on_damaged(world: &mut World, victim: Entity) {
    fire(world, Moment::OnDamaged, victim, None);
}

/// Whether `seen` carries anything that answers being looked at. Asked by the
/// look reticle, which has no blow to hide behind: it wants to know whether
/// pointing the cursor at this creature is going to cost the player their
/// turn.
pub fn answers_being_looked_at(world: &World, seen: Entity) -> bool {
    ABILITIES
        .iter()
        .any(|a| a.when == Moment::OnTargeted && a.armed(world, seen))
}

/// `mob` is about to swing at `target`. Gives every ability armed on the
/// attacker a chance to take the turn instead.
///
/// Reports whether one did. When none does, the caller queues the ordinary
/// blow — which is the whole of what `ai` needs to know, and rather less than
/// it used to know.
pub fn fire_instead_of_attacking(world: &mut World, mob: Entity, target: Entity) -> bool {
    for ability in ABILITIES {
        let Moment::InsteadOfAttacking(chance) = ability.when else {
            continue;
        };
        if !ability.armed(world, mob) {
            continue;
        }
        if !world.resource_mut::<GameRng>().0.gen_bool(chance) {
            continue;
        }
        if (ability.action)(world, mob, Some(target)) {
            return true;
        }
    }
    false
}

/// `looker` has turned their attention on `seen` — attacked, zapped or thrown
/// something at them. Fires before the blow itself and whether or not it
/// lands.
///
/// Note which way round this is: the ability is armed by what `seen` carries,
/// so `seen` is the bearer and `looker` is the other end. A medusa's gaze is
/// something the medusa has, not something the player does.
/// Reports whether anything actually answered — which the look reticle needs,
/// because for it the answer is the whole event rather than a rider on a blow.
pub fn fire_on_targeted(world: &mut World, looker: Entity, seen: Entity) -> bool {
    fire(world, Moment::OnTargeted, seen, Some(looker))
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
/// The odds live on the row, so the roll is made here rather than inside each
/// mechanic — which is why this moment has an entry point of its own rather
/// than simply calling `fire`.
///
/// Assumes `ai`, well upstream, has already moved every monster this turn —
/// the reason a bearer's own jump lands clear at the top of the *next* turn
/// rather than into a monster still mid-move on this one.
pub fn ability_system(world: &mut World) {
    for bearer in actors(world) {
        for ability in ABILITIES {
            let Moment::EachTurn(chance) = ability.when else {
                continue;
            };
            if !ability.armed(world, bearer) {
                continue;
            }
            if !world.resource_mut::<GameRng>().0.gen_bool(chance) {
                continue;
            }
            let did_something = (ability.action)(world, bearer, None);
            let Some(flavour) = ability.flavour else {
                continue;
            };
            if did_something && world.get::<Player>(bearer).is_some() {
                world.resource_mut::<GameLog>().add(flavour.to_string());
            }
        }
    }
}
