use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::abilities::{Blow, cleave_attack, fire_on_hit, fire_on_targeted};
use crate::components::*;
use crate::conditions::afflicted;
use crate::effects::{
    Asleep, Bided, Fencer, Grant, Pinned, Rooted, VorpalOnCondition, VorpalTarget, WhirlOnMove,
    loadout,
};
use crate::equipment::{equipped_items, force_unequip};
use crate::helpers::{
    chebyshev, death_burst, get_line, mob_at, player_sees, spill_blood, took_damage,
};
use crate::identify::display_name;
use crate::map::{GameRng, Map};
use crate::particles::Particles;
use crate::score::award_kill;
use crate::shake::{ShakeKind, kick_shake};
use crate::state::Ending;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   EXCELLENT_HIT_CHANCE / EXCELLENT_HIT_DICE  the player's Nd[power] crit
//   CHIP_DAMAGE                                the player's guaranteed-1 floor
//   GEAR_SURVIVES_DEATH                        per-item odds a corpse keeps its gear
use crate::constants::combat::{
    BIDE_ATTACK_BONUS, CHIP_DAMAGE, EXCELLENT_HIT_CHANCE, EXCELLENT_HIT_DICE, GEAR_SURVIVES_DEATH,
};

/// Rolls `1dN`. A non-positive number of sides means "no die", which rolls 0 so
/// an unarmoured/unarmed entity simply contributes nothing to the opposed roll.
fn roll_die(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    if sides <= 0 {
        return 0;
    }
    rng.gen_range(1..=sides)
}

/// The `bane` of the attacker's currently-wielded weapon, if that weapon has
/// been vorpalized (scroll of vorpalize weapon). `None` for an unarmed attacker
/// or a plain weapon — so monsters, which never wield, are unaffected.
fn wielded_vorpal_bane(world: &World, entity: Entity) -> Option<String> {
    equipped_items(world, entity)
        .into_iter()
        .find_map(|i| world.get::<Vorpal>(i).map(|v| v.bane.clone()))
}

/// Looks up an entity's display name, falling back to a vague noun so the log
/// never prints a raw entity id at the player.
fn entity_name(world: &World, entity: Entity) -> String {
    world
        .get::<Name>(entity)
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "something".to_string())
}

/// The combat schedule step: drains the [`AttackQueue`] and resolves every
/// pending attack (currently these are all monster-initiated; the player's
/// melee is resolved inline by the input handler).
///
/// Assumes `equipment_effects_system` has already folded every lent effect
/// into `Loadout`-relevant components this turn, so the opposed roll below
/// reads a wearer's current gear and never a stale one.
pub fn combat_system(world: &mut World) {
    let attacks = std::mem::take(&mut world.resource_mut::<AttackQueue>().attacks);

    for attack in attacks {
        resolve_attack(world, attack.attacker, attack.target);
    }
}

/// Sweeps up anything that has been reduced to 0 HP by a source that doesn't
/// resolve its own lethality — a wand bolt, and any blast casualty
/// [`crate::items::wands::elemental_blast`] didn't already finish off itself
/// (it does, when it has a blast centre to fling a corpse away from; this is
/// the catch-all for the rest). Melee kills are still finalised inline by
/// [`resolve_attack`], so by the time this runs the only casualties left are
/// indirect ones with no known source to fling a corpse away from.
///
/// Assumes `combat_system` has already run this turn, so any `Fighter.hp <=
/// 0` found here is this turn's business to finish, never a casualty left
/// over from one that already swept.
pub fn reaper_system(world: &mut World) {
    let doomed: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Fighter)>();
        q.iter(world)
            .filter(|(_, f)| f.hp <= 0)
            .map(|(e, _)| e)
            .collect()
    };

    for entity in doomed {
        finish_indirect_kill(world, entity, None);
    }
}

/// A tick of recoil for a creature dying where the player can see it: a kill is
/// worth exactly one frame of punctuation, which is why it takes the short kick
/// rather than the heavy one an excellent hit gets. Gated on sight like every
/// other cosmetic, so a monster dying in a room the player has never entered
/// doesn't announce itself through the floor.
///
/// It never steps on a bigger shake: a blast that killed a whole room is
/// already rocking harder than this, and [`Shake::kick`](crate::shake::Shake::kick)
/// keeps whichever is worth more.
fn kill_shake(world: &mut World, victim: Entity) {
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    if !player_sees(world, pos.x, pos.y) {
        return;
    }
    kick_shake(world, ShakeKind::Kill);
}

/// Blanks an entity's on-screen glyph to a blank space — used only to hide
/// the player's `@` the instant they die, so the death burst's flung corpse
/// and bone shrapnel read as *them* exploding rather than a corpse detaching
/// from a body still visibly standing there. The player entity is never
/// despawned (the death screen still needs it), so there's nothing to
/// restore it: the run is over.
fn blank_player_glyph(world: &mut World, entity: Entity) {
    if let Some(mut r) = world.get_mut::<Renderable>(entity) {
        r.glyph = ' ';
    }
}

/// Finalises one creature that dropped to lethal HP through a source with no
/// attacker entity to report — a wand bolt, or a blast casualty
/// [`crate::items::wands::elemental_blast`] hands off directly rather than
/// waiting for [`reaper_system`]'s next sweep. Plays the death burst, then
/// either despawns a monster (gear settled first, a plain "dies" line
/// logged) or, for the player, blanks their glyph and flags [`Ending`] —
/// guarded so a player already marked dead this run is neither burst nor
/// re-flagged a second time by a later casualty in the same blast.
///
/// `source` is where the corpse should fly away from — a blast's centre, say
/// — or `None` for a random fling direction.
pub(crate) fn finish_indirect_kill(world: &mut World, entity: Entity, source: Option<Position>) {
    if world.get::<Player>(entity).is_some() {
        if world.resource::<Ending>().player_dead {
            return;
        }
        death_burst(world, entity, source);
        blank_player_glyph(world, entity);
        kick_shake(world, ShakeKind::Death);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = "Killer unknown".to_string();
        return;
    }

    let name = entity_name(world, entity);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} dies."));
    pay_for_the_corpse(world, entity);
    kill_shake(world, entity);
    death_burst(world, entity, source);
    leave_gear_behind(world, entity);
    world.despawn(entity);
}

/// What a corpse is worth, paid into the player's score the moment a creature
/// stops being one ([`crate::score::award_kill`]).
///
/// It never asks whose blade it was. Half the ways a monster dies in nihilurk have
/// no swinger to ask about — a bolt, a blast a room away, a trapdoor it walked
/// into — and a scoreboard that paid for some of those and not others would
/// only be teaching the player to kill things in the approved fashion.
fn pay_for_the_corpse(world: &mut World, victim: Entity) {
    let Some(max_hp) = world.get::<Fighter>(victim).map(|f| f.max_hp) else {
        return;
    };
    award_kill(world, max_hp);
}

/// Settles what a dying creature was wearing, item by item. Each piece gets its
/// own [`GEAR_SURVIVES_DEATH`] coin flip: heads it clatters onto the corpse's
/// tile, announced so the player knows there is something to go back for; tails
/// it is destroyed with its owner and never mentioned again.
///
/// This is what stops a thrown dagger an orc caught (see
/// [`crate::items::throw_system`]) from either vanishing silently into the dead
/// entity or coming back every single time.
fn leave_gear_behind(world: &mut World, entity: Entity) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    for item in equipped_items(world, entity) {
        force_unequip(world, item);
        if !world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(GEAR_SURVIVES_DEATH)
        {
            world.entity_mut(item).despawn();
            continue;
        }
        let name = display_name(world, item);
        world.entity_mut(item).insert(pos);
        world
            .resource_mut::<GameLog>()
            .add(format!("The {name} clatters to the floor."));
    }
}

/// Resolves a single opposed-roll attack of `attacker` against `target`.
///
/// Damage is `(1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus)`: the
/// attacker's and defender's roll totals are computed independently and then
/// subtracted. Every equipped source of a [`crate::effects::Modifier`] folds
/// into those four numbers — a weapon's die, an enchantment's flat bonus, a
/// ring of protection's — and this function never learns which kind of item any
/// of them came from. When the *player* is the attacker two extra rules apply:
///
/// * **Excellent hit** — a [`EXCELLENT_HIT_CHANCE`] chance for a clean strike
///   that rolls [`EXCELLENT_HIT_DICE`] weapon dice (`Nd[Power]`) before the
///   armour is subtracted. Always lands for at least [`CHIP_DAMAGE`], whatever
///   the armour roll or a melee cap left it at — a crit is never reported as
///   having done nothing.
/// * **Chip damage** — a non-excellent player swing still deals at least 1
///   damage, even when the armour roll fully absorbs the weapon roll (logged
///   as a "glancing blow"). Unlike an excellent hit, a glancing blow can never
///   be the killing one — it leaves a foe on 1 HP.
///
/// Finally, gear that carries a [`MeleeCap`](crate::effects::MeleeCap) — a bow,
/// a crossbow — clamps the result. A launcher is worth nothing swung, which is
/// what pays for how good it is drawn.
pub fn resolve_attack(world: &mut World, attacker: Entity, target: Entity) {
    // Missing attacker or target: nothing to resolve.
    if world.get_entity(attacker).is_none() || world.get_entity(target).is_none() {
        return;
    }

    // Looking upon a medusa costs you before your blade ever lands — see
    // `crate::abilities::medusa_gaze`.
    fire_on_targeted(world, attacker, target);

    let matchup = fold_matchup(world, attacker, target);
    // Spent the instant it's folded in — hit, glance or miss — so a
    // double-striking estoc or a cleave only ever sees it on the first swing.
    crate::effects::revoke(world, attacker, Grant::of::<Bided>());
    let swing = roll_swing(world, &matchup);
    let swing = clamp_swing(world, target, &matchup, swing);
    let outcome = land_swing(world, attacker, target, &swing);
    let blow = Landed {
        attacker,
        target,
        // Both roles, read once. `fold_matchup` has already asked about the
        // attacker, so asking about the target here is what lets the three
        // stages below never ask the world again.
        attacker_is_player: matchup.attacker_is_player,
        target_is_player: world.get::<Player>(target).is_some(),
        swing,
        outcome,
    };

    // The aftermath, in the order it has to happen: the punctuation while the
    // corpse still has a tile to be flung off, then the log, then the despawn.
    punctuate(world, &blow);
    report_blow(world, &blow);
    settle_the_dead(world, &blow);
}

/// One player melee attack, tricks and all: the plain opposed-roll swing,
/// plus whatever a wielded weapon lends on top of it — an estoc's second
/// strike ([`Fencer`]), a battle axe's cleave ([`crate::effects::Cleaves`]).
/// Both self-check the marker they answer to, so every caller — a walk into a
/// monster's tile, the chain-sickle's free swing — reaches for this and
/// nothing else, exactly the way nothing outside `crate::equipment` has to
/// know a ring exists.
pub fn melee_attack(world: &mut World, attacker: Entity, target: Entity) {
    resolve_attack(world, attacker, target);
    // The player's tricks alone — a monster that steals or catches one of
    // these weapons still just fights the plain way.
    let is_player = world.get::<Player>(attacker).is_some();
    if is_player && world.get::<Fencer>(attacker).is_some() {
        resolve_attack(world, attacker, target);
    }
    if is_player {
        cleave_attack(world, attacker, target);
    }
}

/// Everything a blow is resolved from, folded out of both sides' gear in one
/// pass: the four dice numbers, the ceiling the attacker's gear imposes, and
/// which side is the hero.
///
/// The four numbers have every equipped source of a
/// [`crate::effects::Modifier`] already folded in — a weapon's die, an
/// enchantment's flat bonus, a ring of protection's — and nothing downstream
/// knows which kind of item supplied any of them.
///
/// It is called a matchup rather than the odds because only four of the six
/// fields are odds. The cap is a ceiling and the last is a role, and they ride
/// here because they come off the same pass over the same gear; a name that
/// covered only the dice would have to be apologised for.
struct Matchup {
    power: i32,
    power_bonus: i32,
    armor: i32,
    armor_bonus: i32,
    /// The strictest ceiling the attacker's gear imposes, if any — a bow.
    /// Folded here rather than looked up again in [`clamp_swing`], because it
    /// comes off the same pass the four numbers above do.
    melee_cap: Option<i32>,
    /// Two rules apply only to the hero's own swing: the excellent hit and the
    /// chip-damage floor. Carried here so the three stages below each ask once.
    attacker_is_player: bool,
}

/// A blow that has already happened: who swung at whom, what the dice said,
/// and what it did to the creature on the end of it.
///
/// The three aftermath stages — [`punctuate`], [`report_blow`],
/// [`settle_the_dead`] — each need all of it, and each used to take the same
/// five arguments in the same order. One of them wanted a sixth. This is that
/// argument list, named, so adding to it is a field rather than a fresh
/// parameter threaded through three signatures.
struct Landed {
    attacker: Entity,
    target: Entity,
    /// Read once in [`resolve_attack`], never re-read. Which side is the
    /// player decides the log's voice, the chip floor and who shakes the
    /// screen, so all three stages want it and none should ask again.
    attacker_is_player: bool,
    target_is_player: bool,
    swing: Swing,
    outcome: Outcome,
}

/// One exchange of dice, and what kind of blow it turned out to be.
struct Swing {
    damage: i32,
    /// The hero's `Nd[power]` crit. Never reported as having done nothing.
    excellent: bool,
    /// The armour ate the whole roll and the chip floor is all that got
    /// through. Draws blood, cannot be the killing blow, arms no shake.
    glancing: bool,
}

/// What the blow did to the creature on the end of it.
struct Outcome {
    lethal: bool,
    /// A vorpalized weapon found its bane, or a garrote found a helpless
    /// throat ([`garrote`]). Either way, skips the HP arithmetic entirely.
    vorpal: bool,
    /// The vorpal kill above was specifically the garrote's trick — the log
    /// line and the death flourish read differently from a blade's.
    garrote: bool,
}

/// Reads both sides' [`Fighter`] once and folds their gear in.
///
/// An entity with no `Fighter` at all still swings for a bare `1d1` and
/// defends with nothing, which is what lets a test dummy fight without one.
fn fold_matchup(world: &World, attacker: Entity, target: Entity) -> Matchup {
    let (power, power_bonus) = world
        .get::<Fighter>(attacker)
        .map_or((1, 0), |f| (f.power, f.power_bonus));
    let (armor, armor_bonus) = world
        .get::<Fighter>(target)
        .map_or((0, 0), |f| (f.armor, f.armor_bonus));
    // One pass over each side's gear rather than one per number. `Fighter`
    // carries the creature's own dice, and the `Loadout` carries what it is
    // wearing; a monster's innate `PowerBonus` lands in the second, which is
    // why the two are added rather than one of them chosen.
    let attackers = loadout(world, attacker);
    let targets = loadout(world, target);
    Matchup {
        power: power + attackers.power_die,
        // A rapier's built-up momentum rides in on top of its own enchantment
        // plus — see `crate::effects::Momentum`. A coiled Bide rides in with
        // it, one blow's worth — see `crate::components::Bided`.
        power_bonus: power_bonus
            + attackers.power_bonus
            + attackers.momentum
            + if world.get::<Bided>(attacker).is_some() {
                BIDE_ATTACK_BONUS
            } else {
                0
            },
        armor: armor + targets.armor_die,
        armor_bonus: armor_bonus + targets.armor_bonus,
        melee_cap: attackers.melee_cap,
        attacker_is_player: world.get::<Player>(attacker).is_some(),
    }
}

/// The two opposed rolls, made independently, and the player-only chip floor
/// under the difference.
///
/// The chip floor is why a fight against good armour is a grind rather than a
/// stalemate: the hero always scrapes off [`CHIP_DAMAGE`], and the blow is
/// flagged `glancing` so everything downstream knows the armour won anyway. An
/// excellent hit is deliberately not this — it is a good roll that happened to
/// net low, not a whiff — so it skips the flag and takes its own floor in
/// [`clamp_swing`].
fn roll_swing(world: &mut World, matchup: &Matchup) -> Swing {
    let mut rng = world.resource_mut::<GameRng>();
    let excellent = matchup.attacker_is_player && rng.0.gen_bool(EXCELLENT_HIT_CHANCE);
    let dice = if excellent { EXCELLENT_HIT_DICE } else { 1 };
    let attack_total: i32 = (0..dice)
        .map(|_| roll_die(&mut rng.0, matchup.power))
        .sum::<i32>()
        + matchup.power_bonus;
    let armor_roll = roll_die(&mut rng.0, matchup.armor) + matchup.armor_bonus;

    let net = attack_total - armor_roll;
    let glancing = matchup.attacker_is_player && !excellent && net < CHIP_DAMAGE;
    let damage = match glancing {
        true => CHIP_DAMAGE,
        false => net.max(0),
    };
    Swing {
        damage,
        excellent,
        glancing,
    }
}

/// The three ceilings and floors that sit on top of the dice, **in this order**
/// — each one is written the way it is because of the one before it:
///
/// 1. A glancing blow can leave a foe on 1 HP but never take the last point.
///    Chip damage exists so a turned-aside swing isn't *nothing*, not so it
///    finishes people.
/// 2. A [`MeleeCap`](crate::effects::MeleeCap) clamps whatever is left. It is
///    applied after the floor so a cap of 0 really is 0 — a bow swung is worth
///    nothing, which is the price of the hand it occupies.
/// 3. An excellent hit is never reported as having done nothing, whatever the
///    armour roll or the cap left it at.
fn clamp_swing(world: &World, target: Entity, matchup: &Matchup, mut swing: Swing) -> Swing {
    if let Some(f) = world.get::<Fighter>(target).filter(|_| swing.glancing) {
        swing.damage = swing.damage.min((f.hp - 1).max(0));
    }
    if let Some(cap) = matchup.melee_cap {
        swing.damage = swing.damage.min(cap);
    }
    if swing.excellent {
        swing.damage = swing.damage.max(CHIP_DAMAGE);
    }
    swing
}

/// Takes the HP off, and everything that happens *because* a blow connected:
/// the vorpal shear, the attacker's own on-hit magic, the blood, the broken
/// promise and the low-HP warning.
///
/// Melee is the one damage path that applies its own HP change, so it has to
/// ask [`crate::helpers::took_damage`] for the rest by hand; every other path
/// gets it from `helpers::apply_damage`.
fn land_swing(world: &mut World, attacker: Entity, target: Entity, swing: &Swing) -> Outcome {
    // A vorpalized weapon that draws blood slays its bane outright — and any
    // creature carrying `VorpalTarget` (the Jabberwock), whatever the bane. A
    // glancing scrape never triggers it.
    let blade_vorpal = wielded_vorpal_bane(world, attacker).is_some_and(|bane| {
        world.get::<VorpalTarget>(target).is_some()
            || world.get::<Name>(target).is_some_and(|n| n.what == bane)
    });
    // The garrote's own trick: a target already helpless with a negative
    // condition dies to any hit at all, whatever its weapon class — even a
    // glancing one. A vorpalized blade still needs a real, non-glancing hit
    // to draw the blood its bane dies to.
    let garrote = garrote_vorpal(world, attacker, target);
    let vorpal = swing.damage > 0 && (garrote || (!swing.glancing && blade_vorpal));
    let garrote = garrote && vorpal;

    let hp_before = world.get::<Fighter>(target).map(|f| f.hp);
    let mut lethal = false;
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.hp -= swing.damage;
        if vorpal {
            fighter.hp = 0;
        }
        lethal = fighter.hp <= 0;
    }

    if swing.damage > 0 {
        // Whatever the attacker's own magic does to something it just hit — a
        // charmed pair of hands passing its confusion on, an aquator's touch
        // eating the armour. One table (`abilities::ABILITIES`), and
        // combat never learns what is in it: it only says what kind of blow
        // this was.
        let blow = Blow {
            glancing: swing.glancing,
            lethal,
        };
        fire_on_hit(world, attacker, target, blow);
        spill_blood(world, target, swing.damage, swing.glancing);
        took_damage(world, target, hp_before);
    }

    Outcome {
        lethal,
        vorpal,
        garrote,
    }
}

/// Whether `attacker`'s garrote finds a helpless throat: it's wielding one
/// ([`VorpalOnCondition`], lent to the wielder while it's in hand — see
/// [`crate::catalog::WeaponDef::grants`]) and `target` is carrying a negative
/// condition — the same afflictions [`crate::conditions::afflicted`] answers
/// for, plus a snare: pinned, held or asleep is exactly as helpless. A
/// monster's own confusion never gets the [`Confused`] component `afflicted`
/// checks — [`crate::conditions::stagger`] tags it on [`Mob::movement_type`]
/// instead — so that's checked here directly.
/// The player's trick alone — a monster that steals or catches a garrote
/// still just fights the plain way.
fn garrote_vorpal(world: &World, attacker: Entity, target: Entity) -> bool {
    let mob_confused = world
        .get::<Mob>(target)
        .is_some_and(|m| matches!(m.movement_type, MovementType::Confused));
    world.get::<Player>(attacker).is_some()
        && world.get::<VorpalOnCondition>(attacker).is_some()
        && (afflicted(world, target)
            || mob_confused
            || world.get::<Asleep>(target).is_some()
            || world.get::<Pinned>(target).is_some()
            || world.get::<Rooted>(target).is_some())
}

/// The estoc's lunge, end to end: self-checks [`Fencer`] and the geometry —
/// one empty tile dead ahead, an enemy past it — and, if both hold, resolves
/// the guaranteed strike and carries `attacker` forward into the tile it just
/// closed. Returns whether it fired, so the engine's own step (a plain walk)
/// knows to stand down.
///
/// The one thing this can't check for itself is which way `attacker` is
/// moving — `(dx, dy)` is the step already decided upstream, one tile in any
/// of the eight directions.
pub fn try_lunge(world: &mut World, attacker: Entity, dx: i16, dy: i16) -> bool {
    if world.get::<Player>(attacker).is_none() || world.get::<Fencer>(attacker).is_none() {
        return false;
    }
    let Some(origin) = world.get::<Position>(attacker).copied() else {
        return false;
    };
    let near = Position {
        x: origin.x.saturating_add_signed(dx),
        y: origin.y.saturating_add_signed(dy),
    };
    let far = Position {
        x: near.x.saturating_add_signed(dx),
        y: near.y.saturating_add_signed(dy),
    };
    let near_clear = {
        let map = world.resource::<Map>();
        !map.blocks(near.x, near.y) && map.diagonal_step_ok(origin.x, origin.y, near.x, near.y)
    } && mob_at(world, near).is_none();
    let Some(target) = near_clear.then(|| mob_at(world, far)).flatten() else {
        return false;
    };

    resolve_lunge(world, attacker, target);
    if let Some(mut pos) = world.get_mut::<Position>(attacker) {
        *pos = near;
    }
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(attacker) {
        viewshed.dirty = true;
    }
    world.entity_mut(attacker).insert(EntityMoved);
    true
}

/// The chain-sickle's whirl: self-checks [`WhirlOnMove`] and finds a [`Mob`]
/// adjacent to both `old` and `new` — a step taken alongside an enemy rather
/// than toward or away from it — and lands a free [`melee_attack`] on it if
/// one qualifies. A no-op for anyone not wielding one.
pub fn try_whirl_attack(world: &mut World, attacker: Entity, old: Position, new: Position) {
    if world.get::<Player>(attacker).is_none() || world.get::<WhirlOnMove>(attacker).is_none() {
        return;
    }
    let target = {
        let mut query = world.query_filtered::<(Entity, &Position), With<Mob>>();
        query
            .iter(world)
            .find(|&(_, &p)| chebyshev(p, old) <= 1 && chebyshev(p, new) <= 1)
            .map(|(e, _)| e)
    };
    if let Some(target) = target {
        melee_attack(world, attacker, target);
    }
}

/// Resolves the estoc's lunge: closing the last stride of a run lands a
/// guaranteed strike at triple the normal weapon roll, armour ignored
/// outright — the promise a thin blade makes that a plate-armoured swing
/// can't. Called by [`try_lunge`] once it has confirmed the geometry, in
/// place of the ordinary walk.
fn resolve_lunge(world: &mut World, attacker: Entity, target: Entity) {
    if world.get_entity(attacker).is_none() || world.get_entity(target).is_none() {
        return;
    }
    fire_on_targeted(world, attacker, target);

    let matchup = fold_matchup(world, attacker, target);
    let damage = {
        let mut rng = world.resource_mut::<GameRng>();
        (0..3)
            .map(|_| roll_die(&mut rng.0, matchup.power))
            .sum::<i32>()
            + matchup.power_bonus * 3
    }
    .max(1);
    let swing = Swing {
        damage,
        excellent: false,
        glancing: false,
    };
    let outcome = land_swing(world, attacker, target, &swing);
    let blow = Landed {
        attacker,
        target,
        attacker_is_player: matchup.attacker_is_player,
        target_is_player: world.get::<Player>(target).is_some(),
        swing,
        outcome,
    };
    punctuate(world, &blow);

    let target_name = entity_name(world, target);
    world.resource_mut::<GameLog>().add(format!(
        "You lunge, blade flashing past every guard, and skewer the {target_name} for {damage} damage!"
    ));
    if blow.outcome.lethal {
        world
            .resource_mut::<GameLog>()
            .add(format!("You have slain the {target_name}!"));
    }
    settle_the_dead(world, &blow);
}

/// A reach weapon's strike (a bardiche, a whip): traces the line from
/// `attacker` out to `at`, capped at `weapon`'s own [`Reach`], and resolves an
/// ordinary [`resolve_attack`] against the first creature it finds — or, for a
/// [`ReachPiercing`] weapon, every creature standing in it. A wall stops the
/// line short the way it stops a thrown missile.
pub fn resolve_reach_attack(world: &mut World, attacker: Entity, weapon: Entity, at: Position) {
    // The player's own reticle alone — a monster in melee range of a wielded
    // bardiche or whip just swings it the plain way.
    if world.get::<Player>(attacker).is_none() {
        return;
    }
    let Some(&Reach(reach)) = world.get::<Reach>(weapon) else {
        return;
    };
    let Some(origin) = world.get::<Position>(attacker).copied() else {
        return;
    };
    let piercing = world.get::<ReachPiercing>(weapon).is_some();
    let map = world.resource::<Map>().clone();

    let mut cells = Vec::new();
    let mut victims = Vec::new();
    for (steps, pos) in get_line(origin, at).into_iter().enumerate() {
        if pos == origin {
            continue;
        }
        if steps as i32 > reach || map.blocks(pos.x, pos.y) {
            break;
        }
        cells.push((pos.x, pos.y));
        let hit = world
            .query_filtered::<(Entity, &Position), Or<(With<Mob>, With<Player>)>>()
            .iter(world)
            .find(|(e, p)| *e != attacker && **p == pos)
            .map(|(e, _)| e);
        if let Some(victim) = hit {
            victims.push(victim);
            if !piercing {
                break;
            }
        }
    }

    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.hurl(&cells, '-', Color::Cyan);
    }

    if victims.is_empty() {
        world
            .resource_mut::<GameLog>()
            .add("You strike at nothing but air.".to_string());
        return;
    }
    for victim in victims {
        resolve_attack(world, attacker, victim);
    }
}

/// The spark a blow leaves on the tile it landed on. Every blow leaves exactly
/// one, which is why this is an enum rather than three flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spark {
    /// Nothing got through the armour: a grey dot, and no blood.
    Nothing,
    /// A glancing blow scrapes off its chip of HP without ever getting through
    /// the armour, and the spark says so: it is the one hit that draws blood
    /// and still earns no kick.
    Glance,
    /// A blow that got through.
    Hit,
}

/// What a blow earns the screen, decided from the numbers and nothing else.
///
/// This is what lets [`punctuate`] apply a plan rather than pile up adjacent
/// conditionals. The *what* is a pure function of the dice and the *when* is
/// all that is left downstream.
///
/// Deliberately not unit-tested, even though being pure makes it easy to test.
/// Everything it decides is a spark glyph and a screen shake, and nihilurk does not
/// hold its cosmetics to automated tests — they are checked by playing, which
/// is the only thing that can tell whether a kick reads as a kick. Note the one
/// rule that is not obvious from any single line: `strike` and `kill_kick` are
/// independent, so an excellent killing blow fires **both**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Flourish {
    /// Where the blow landed.
    spark: Spark,
    /// The kick the *strike* is worth. `None` for a glancing scrape, for every
    /// monster's swing, and for a killing blow — which takes `kill_kick`
    /// instead.
    strike: Option<ShakeKind>,
    /// A kill's own, shorter kick. Deliberately *not* exclusive with `strike`:
    /// an excellent killing blow fires the heavy thump for the strike and this
    /// one for the death, and two kicks is the intended answer.
    kill_kick: bool,
    /// The Mortal-Kombat-style death flourish — flung corpse, bone shrapnel, a
    /// wall splatter if it earns one. Unlike `kill_kick`, this fires for the
    /// player's own death too.
    burst: bool,
}

impl Flourish {
    /// The whole decision, from the dice.
    fn of(blow: &Landed) -> Self {
        Self {
            spark: match (blow.swing.damage, blow.swing.glancing) {
                (0, _) => Spark::Nothing,
                (_, true) => Spark::Glance,
                _ => Spark::Hit,
            },
            strike: Self::strike_kick(blow),
            // A monster that kills the player takes the `Death` lurch in
            // `settle_the_dead`, which is a bigger one than this.
            kill_kick: blow.outcome.lethal && !blow.target_is_player,
            burst: blow.outcome.lethal,
        }
    }

    /// The kick for the swing itself, as opposed to the one for the death.
    fn strike_kick(blow: &Landed) -> Option<ShakeKind> {
        // A thump through the whole map for the one swing in seven that lands
        // clean...
        if blow.swing.excellent {
            return Some(ShakeKind::Heavy);
        }
        // ...and the lightest kick in the set for every other swing of the
        // player's that got through armour. Three exclusions, and each is
        // somebody else's kick or nobody's: a crit took the heavy one above, a
        // kill takes its own, and a glancing scrape is the game saying the
        // armour ate the blow.
        let ordinary = blow.attacker_is_player
            && !blow.swing.glancing
            && !blow.outcome.lethal
            && blow.swing.damage > 0;
        ordinary.then_some(ShakeKind::Hit)
    }
}

/// Everything that punctuates the blow: a spark saying *where* it landed, a
/// kick saying how hard, and the death flourish if it killed.
///
/// All of it needs `target` to still have a tile, so this runs before anything
/// despawns it. Only the player's own hits kick the screen; a monster's blow
/// reaches the map through the low-HP crossing, or not at all.
///
/// The effects stay in one function because they share one window: every one
/// of them has to happen *after* the HP is off — a kill kick has to know it
/// was a kill — and *before* the despawn, because a burst needs the corpse's
/// tile. That window is the two statements between `land_swing` and
/// `settle_the_dead` in [`resolve_attack`]; four one-line functions sharing it
/// would be four places to get the ordering wrong instead of one.
fn punctuate(world: &mut World, blow: &Landed) {
    let flourish = Flourish::of(blow);
    let attacker_pos = world.get::<Position>(blow.attacker).copied();

    if let Some(tpos) = world.get::<Position>(blow.target).copied() {
        // The effect layer is optional (tests run without it).
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            match flourish.spark {
                Spark::Nothing => fx.blip(tpos.x, tpos.y, '·', Color::DarkGrey),
                Spark::Glance => fx.clink_spark(tpos.x, tpos.y),
                Spark::Hit => fx.hit_spark(tpos.x, tpos.y),
            }
        }
    }
    if let Some(kind) = flourish.strike {
        kick_shake(world, kind);
    }
    // Asked for while the corpse still has the Position the sight gate reads.
    if flourish.kill_kick {
        kill_shake(world, blow.target);
    }
    if flourish.burst {
        death_burst(world, blow.target, attacker_pos);
    }
    // The garrote's own flourish: a helpless victim doesn't fall so much as
    // pop — a wide, wet burst on top of the ordinary death fling.
    if blow.outcome.garrote {
        if let Some(tpos) = world.get::<Position>(blow.target).copied() {
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.spark_burst(tpos.x, tpos.y, Color::Red);
            }
            kick_shake(world, ShakeKind::Heavy);
        }
    }
}

/// The two or three lines the log gets, from whichever end of the blow the
/// player was on.
fn report_blow(world: &mut World, blow: &Landed) {
    let attacker_name = entity_name(world, blow.attacker);
    let target_name = entity_name(world, blow.target);
    let target_is_player = blow.target_is_player;
    // An attacker the player can't see — an invisible phantom, or a mob still
    // off in the dark — is reported only as "Something".
    let attacker_unseen = target_is_player && world.get::<Hidden>(blow.attacker).is_some();

    let mut log = world.resource_mut::<GameLog>();
    if blow.attacker_is_player {
        return report_player_hit(&mut log, &target_name, &blow.swing, &blow.outcome);
    }
    let target_label = if target_is_player {
        "you".to_string()
    } else {
        format!("the {target_name}")
    };
    let atk = if attacker_unseen {
        "Something".to_string()
    } else {
        format!("The {attacker_name}")
    };
    report_monster_hit(
        &mut log,
        &atk,
        &target_label,
        &target_name,
        blow.swing.damage,
        blow.outcome.lethal,
        target_is_player,
    );
}

/// What is left of a creature the blow killed. A monster pays its score, drops
/// what survives it and leaves the world; the player does none of those things
/// — they stay in it, because the death screen still needs them.
fn settle_the_dead(world: &mut World, blow: &Landed) {
    if !blow.outcome.lethal {
        return;
    }
    if blow.target_is_player {
        // The main loop notices the `Ending` resource, tears down the save and
        // shows the death screen. Their `@` is blanked so the death burst's
        // flung corpse reads as them exploding rather than detaching from a
        // body still standing there, and the map takes the biggest lurch it
        // has in it on the way out.
        let attacker_name = entity_name(world, blow.attacker);
        blank_player_glyph(world, blow.target);
        kick_shake(world, ShakeKind::Death);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = format!("Slain by the {attacker_name}");
        return;
    }
    pay_for_the_corpse(world, blow.target);
    leave_gear_behind(world, blow.target);
    world.despawn(blow.target);
}

/// Writes the player-attacked-something lines to the log: the hit line (an
/// excellent hit, a glancing scrape, or a plain blow) and, on a kill, the
/// vorpal flourish and the slain line.
fn report_player_hit(log: &mut GameLog, target_name: &str, swing: &Swing, outcome: &Outcome) {
    match (swing.excellent, swing.glancing) {
        (true, _) => log.add(format!(
            "You score an excellent hit on the {target_name} for {} damage!",
            swing.damage
        )),
        (_, true) => log.add(format!("You deal a glancing blow to the {target_name}.")),
        _ => log.add(format!(
            "You hit the {target_name} for {} damage.",
            swing.damage
        )),
    }
    if outcome.lethal && outcome.garrote {
        log.add(format!(
            "You choke the life out of the helpless {target_name}! Atrocious!"
        ));
    } else if outcome.lethal && outcome.vorpal {
        log.add(format!(
            "Snicker-snack! The blade shears clean through the {target_name}!"
        ));
    }
    if outcome.lethal {
        log.add(format!("You have slain the {target_name}!"));
    }
}

/// Writes the something-attacked-a-creature lines to the log. `atk` and
/// `target_label` are pre-rendered ("The orc" / "Something", "you" / "the rat")
/// so this never has to know whether the player was on either end.
fn report_monster_hit(
    log: &mut GameLog,
    atk: &str,
    target_label: &str,
    target_name: &str,
    damage: i32,
    lethal: bool,
    target_is_player: bool,
) {
    match damage {
        0 => log.add(format!("{atk} misses {target_label}.")),
        _ => log.add(format!("{atk} hits {target_label} for {damage} damage.")),
    }
    if lethal && target_is_player {
        log.add(format!("{atk} strikes you down..."));
    }
    if lethal && !target_is_player {
        log.add(format!("{atk} kills the {target_name}!"));
    }
}
