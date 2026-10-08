//! One blow, start to finish: the opposed roll, the weapon tricks riding on
//! top of it, and everything a landed hit sets off — the log lines, the
//! sparks and shakes, and what is left of a creature it killed.
//!
//! [`resolve_attack`] is the one place damage is decided; [`melee_attack`] is
//! what a player's swing actually calls, since it also has to ask a wielded
//! weapon whether it strikes twice or cleaves. Everything past the dice is
//! folded into [`Landed`] once and threaded through the three aftermath
//! stages — [`punctuate`], [`report_blow`], [`settle_the_dead`] — so each
//! reads the blow rather than re-deriving it.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::abilities::{Blow, cleave_attack, fire_on_hit, fire_on_struck, fire_on_targeted};
use crate::components::*;
use crate::conditions::afflicted;
use crate::constants::score::BOUNTY_SCORE_MULTIPLIER;
use crate::effects::{
    Asleep, Bided, Binds, Clamped, ClampedBy, Fencer, Grant, Lunges, Lurk, Pinned, Rooted,
    ScoreBounty, ShattersStone, VorpalOnCondition, VorpalTarget, WhirlOnMove, loadout, revoke,
};
use crate::equipment::{equipped_items, force_unequip};
use crate::helpers::{
    chebyshev, death_burst, get_line, mob_at, monster_at, player_sees, spill_blood, took_damage,
};
use crate::identify::display_name;
use crate::map::{GameRng, Map};
use crate::particles::Particles;
use crate::score::award_kill;
use crate::shake::{Shake, ShakeKind, kick_shake};
use crate::state::Ending;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   EXCELLENT_HIT_CHANCE / EXCELLENT_HIT_DICE  the player's Nd[power] crit
//   CHIP_DAMAGE                                the player's guaranteed-1 floor
//   GEAR_SURVIVES_DEATH                        per-item odds a corpse keeps its gear
use crate::constants::combat::{
    BELL_CURVE_DICE, BIDE_ATTACK_BONUS, CHIP_DAMAGE, EXCELLENT_HIT_CHANCE, EXCELLENT_HIT_DICE,
    GEAR_SURVIVES_DEATH,
};
use crate::constants::decks::{BALA_POWER, BOLE_ARMOR};

/// Rolls `1dN`. A non-positive number of sides means "no die", which rolls 0 so
/// an unarmoured/unarmed entity simply contributes nothing to the opposed roll.
fn roll_die(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    if sides <= 0 {
        return 0;
    }
    rng.gen_range(1..=sides)
}

/// Rolls `1dN` [`BELL_CURVE_DICE`] times and averages, rounding down. Same
/// range and mean as a plain [`roll_die`], just a narrower distribution
/// around that mean — the bell curve that takes the swing out of a normal
/// exchange without touching any weapon or armour's tuned die size.
pub(crate) fn roll_die_bell(rng: &mut ChaCha12Rng, sides: i32) -> i32 {
    (0..BELL_CURVE_DICE)
        .map(|_| roll_die(rng, sides))
        .sum::<i32>()
        / BELL_CURVE_DICE
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
    if let Some(form) = crate::effects::chimeric_form(world, entity) {
        return form.name.to_string();
    }
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
        if world.get_entity(entity).is_none() {
            continue;
        }
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

/// Stops the map dead for the rest of the run, called from both player-death
/// paths. A player's death arms no shake of its own, but the blow that killed
/// them may have armed one a frame earlier — a blast's `Heavy`, or the
/// `Wounded` kick from the crossing on the way down — and that one would still
/// be rocking over the slow death burst. Switching the layer off rather than
/// only settling it also means a casualty later in the same blast can't arm a
/// fresh one over a corpse.
fn silence_shake(world: &mut World) {
    if let Some(mut shake) = world.get_resource_mut::<Shake>() {
        shake.enabled = false;
        shake.settle();
    }
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
        silence_shake(world);
        death_burst(world, entity, source);
        blank_player_glyph(world, entity);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = strings::killer_unknown().to_string();
        return;
    }

    if let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    {
        release_biters_grip(world, entity, player);
    }
    if reveal_faerie(world, entity) {
        return;
    }
    let name = entity_name(world, entity);
    world
        .resource_mut::<GameLog>()
        .add(strings::mob_dies(&name));
    match world.get::<Helper>(entity).is_some() {
        true => crate::companion::mourn(world, entity),
        false => pay_for_the_corpse(world, entity),
    }
    kill_shake(world, entity);
    death_burst(world, entity, source);
    leave_gear_behind(world, entity);
    burst_on_death(world, entity);
    crate::spirits::poof(world, entity);
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
    let mult = match world.get::<ScoreBounty>(victim) {
        Some(_) => BOUNTY_SCORE_MULTIPLIER,
        None => 1,
    };
    award_kill(world, max_hp * mult);
    crate::body::feed(world);
}

/// Settles what a dying creature was wearing, item by item. Each piece gets its
/// own [`GEAR_SURVIVES_DEATH`] coin flip: heads it clatters onto the corpse's
/// tile, announced so the player knows there is something to go back for; tails
/// it is destroyed with its owner and never mentioned again.
///
/// This is what stops a thrown dagger an orc caught (see
/// [`crate::items::throw_system`]) from either vanishing silently into the dead
/// entity or coming back every single time.
pub(crate) fn leave_gear_behind(world: &mut World, entity: Entity) {
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
            .add(strings::gear_clatters_to_floor(&name));
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
/// * **Chip damage** — a non-excellent player swing still deals at least
///   [`CHIP_DAMAGE`], even when the armour roll fully absorbs the weapon roll (logged
///   as a "glancing blow"). Unlike an excellent hit, a glancing blow can never
///   be the killing one — it leaves a foe on 1 HP.
///
/// Finally, gear that carries a [`MeleeCap`](crate::effects::MeleeCap) — a bow,
/// a crossbow — clamps the result. A launcher is worth nothing swung, which is
/// what pays for how good it is drawn.
pub fn resolve_attack(world: &mut World, attacker: Entity, target: Entity) {
    if world.get_entity(attacker).is_none() || world.get_entity(target).is_none() {
        return;
    }

    if world.get::<Player>(attacker).is_some() && crate::spirits::is_peaceful_spirit(world, target)
    {
        crate::spirits::trigger_event(world, attacker, target);
        return;
    }

    fire_on_targeted(world, attacker, target);

    let matchup = fold_matchup(world, attacker, target);
    crate::effects::revoke(world, attacker, Grant::of::<Bided>());
    let swing = roll_swing(world, &matchup);
    let swing = clamp_swing(world, attacker, target, &matchup, swing);
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
    /// Every blow in this exchange is excellent, whoever swings: the attacker
    /// holds [`crate::effects::Crit`] or the target holds
    /// [`crate::effects::Oof`] (a deck's Chariot, either way up).
    always_excellent: bool,
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
    /// Set when the target was stone and the blow landed on it: the one line
    /// this blow is reported with, in place of the hit line
    /// ([`crate::effects::stone_chip`]).
    chipped: Option<String>,
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
    let attackers = loadout(world, attacker);
    let targets = loadout(world, target);
    Matchup {
        power: power + attackers.power_die,
        power_bonus: power_bonus
            + attackers.power_bonus
            + attackers.momentum
            + if world.get::<Bided>(attacker).is_some() {
                BIDE_ATTACK_BONUS
            } else {
                0
            }
            + if world.get::<crate::effects::Bala>(attacker).is_some() {
                BALA_POWER
            } else {
                0
            },
        armor: armor + targets.armor_die,
        armor_bonus: armor_bonus
            + targets.armor_bonus
            + if world.get::<crate::effects::Bole>(target).is_some() {
                BOLE_ARMOR
            } else {
                0
            },
        melee_cap: attackers.melee_cap,
        attacker_is_player: world.get::<Player>(attacker).is_some(),
        always_excellent: world.get::<crate::effects::Crit>(attacker).is_some()
            || world.get::<crate::effects::Oof>(target).is_some(),
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
    let excellent = matchup.always_excellent
        || (matchup.attacker_is_player && rng.0.gen_bool(EXCELLENT_HIT_CHANCE));
    let attack_total: i32 = if excellent {
        (0..EXCELLENT_HIT_DICE)
            .map(|_| roll_die(&mut rng.0, matchup.power))
            .sum::<i32>()
    } else {
        roll_die_bell(&mut rng.0, matchup.power)
    } + matchup.power_bonus;
    let armor_roll = roll_die_bell(&mut rng.0, matchup.armor) + matchup.armor_bonus;

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
        chipped: None,
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
///
/// Stone comes before all three: a [`crate::effects::Petrified`] target takes
/// a chip at most and never its last point, whatever the dice said, and the
/// blow is reported as the chip it was.
fn clamp_swing(
    world: &World,
    attacker: Entity,
    target: Entity,
    matchup: &Matchup,
    mut swing: Swing,
) -> Swing {
    if world.get::<crate::effects::Protected>(target).is_some() {
        return Swing {
            damage: 0,
            excellent: false,
            glancing: false,
            chipped: None,
        };
    }
    if chip_on_stone(world, attacker, target, &mut swing) {
        return swing;
    }
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

/// Puts [`crate::effects::stone_chip`] on a swing: what a petrified target
/// actually takes, and the line it is reported with. Returns whether the
/// target was stone, which is also the answer to "is this swing settled" —
/// nothing further may raise it.
///
/// Both the ordinary swing and the estoc's lunge come through here, because
/// the lunge builds its own [`Swing`] and would otherwise be the one blow in
/// the game that went through a statue without asking.
///
/// A war hammer ([`ShattersStone`]) is the one thing stone does not stop: its
/// wielder's blow lands whole, and this reports no chip because there was
/// none.
fn chip_on_stone(world: &World, attacker: Entity, target: Entity, swing: &mut Swing) -> bool {
    if world.get::<ShattersStone>(attacker).is_some() {
        return false;
    }
    let Some(chip) = crate::effects::stone_chip(world, target, swing.damage) else {
        return false;
    };
    let landed = swing.damage > 0;
    swing.damage = chip.through;
    if landed {
        swing.glancing = true;
        swing.chipped = Some(chip.line);
    }
    true
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
    let garrote = garrote_vorpal(world, attacker, target);
    let vorpal = swing.damage > 0
        && swing.chipped.is_none()
        && (garrote || (!swing.glancing && blade_vorpal));
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
        let blow = Blow {
            glancing: swing.glancing,
            lethal,
        };
        fire_on_hit(world, attacker, target, blow);
        fire_on_struck(world, target, attacker);
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
/// monster's own confusion or flight never gets a component `afflicted`
/// checks — [`crate::conditions::stagger`] and a scare both tag
/// [`Mob::movement_type`] instead — so both are checked here directly. A
/// fleeing monster isn't truly helpless, but a garrote through the back is
/// the reward for having scared it off in the first place.
/// The player's trick alone — a monster that steals or catches a garrote
/// still just fights the plain way.
fn garrote_vorpal(world: &World, attacker: Entity, target: Entity) -> bool {
    let mob_staggering = world
        .get::<Mob>(target)
        .is_some_and(|m| matches!(m.movement_type, MovementType::Confused | MovementType::Flee));
    world.get::<Player>(attacker).is_some()
        && world.get::<VorpalOnCondition>(attacker).is_some()
        && (afflicted(world, target)
            || mob_staggering
            || world.get::<Asleep>(target).is_some()
            || world.get::<Pinned>(target).is_some()
            || world.get::<Rooted>(target).is_some()
            || world.get::<Clamped>(target).is_some())
}

/// The estoc's lunge, end to end: self-checks [`Lunges`] and the geometry —
/// one empty tile dead ahead, an enemy past it — and, if both hold, resolves
/// the guaranteed strike and carries `attacker` forward into the tile it just
/// closed. Returns whether it fired, so the engine's own step (a plain walk)
/// knows to stand down.
///
/// The one thing this can't check for itself is which way `attacker` is
/// moving — `(dx, dy)` is the step already decided upstream, one tile in any
/// of the eight directions.
pub fn try_lunge(world: &mut World, attacker: Entity, dx: i16, dy: i16) -> bool {
    if world.get::<Player>(attacker).is_none() || world.get::<Lunges>(attacker).is_none() {
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
    let swims = world.get::<crate::effects::Swims>(attacker).is_some();
    let near_clear = {
        let map = world.resource::<Map>();
        map.walkable(near.x, near.y, swims)
            && map.diagonal_step_ok(origin.x, origin.y, near.x, near.y)
    } && mob_at(world, near).is_none();
    let Some(target) = near_clear.then(|| monster_at(world, far)).flatten() else {
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

/// The chain-sickle's whirl: self-checks [`WhirlOnMove`] and finds a monster
/// adjacent to both `old` and `new` — a step taken alongside an enemy rather
/// than toward or away from it — and lands a free [`melee_attack`] on it if
/// one qualifies. A no-op for anyone not wielding one.
pub fn try_whirl_attack(world: &mut World, attacker: Entity, old: Position, new: Position) {
    if world.get::<Player>(attacker).is_none() || world.get::<WhirlOnMove>(attacker).is_none() {
        return;
    }
    let target = {
        let mut query = world.query_filtered::<(Entity, &Position, &Faction), With<Mob>>();
        query
            .iter(world)
            .find(|&(_, &p, &f)| {
                f == Faction::Monster && chebyshev(p, old) <= 1 && chebyshev(p, new) <= 1
            })
            .map(|(e, _, _)| e)
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
    let mut swing = Swing {
        damage,
        excellent: false,
        glancing: false,
        chipped: None,
    };
    chip_on_stone(world, attacker, target, &mut swing);
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

    if let Some(line) = blow.swing.chipped.clone() {
        world.resource_mut::<GameLog>().add(line);
        settle_the_dead(world, &blow);
        return;
    }
    let target_name = entity_name(world, target);
    let line = if world.get::<Lurk>(attacker).is_some() {
        strings::lunge_hit_lurk(&target_name, damage)
    } else {
        strings::lunge_hit(&target_name, damage)
    };
    world.resource_mut::<GameLog>().add(line);
    if blow.outcome.lethal && world.get::<crate::effects::FaerieOnDeath>(target).is_none() {
        world
            .resource_mut::<GameLog>()
            .add(strings::you_have_slain(&target_name));
    }
    settle_the_dead(world, &blow);
}

/// A reach weapon's strike (a bardiche, a whip): traces the line from
/// `attacker` out to `at`, capped at `weapon`'s own [`Reach`], and resolves an
/// ordinary [`resolve_attack`] against the first creature it finds — or, for a
/// [`ReachPiercing`] weapon, every creature standing in it. A wall stops the
/// line short the way it stops a thrown missile.
pub fn resolve_reach_attack(world: &mut World, attacker: Entity, weapon: Entity, at: Position) {
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
            .add(strings::strike_at_nothing());
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
            kill_kick: blow.outcome.lethal && !blow.target_is_player,
            burst: blow.outcome.lethal,
        }
    }

    /// The kick for the swing itself, as opposed to the one for the death.
    fn strike_kick(blow: &Landed) -> Option<ShakeKind> {
        if blow.swing.excellent {
            return Some(ShakeKind::Heavy);
        }
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
    let faerie = world
        .get::<crate::effects::FaerieOnDeath>(blow.target)
        .is_some();
    if flourish.kill_kick && !faerie {
        kill_shake(world, blow.target);
    }
    if flourish.burst && !faerie {
        death_burst(world, blow.target, attacker_pos);
    }
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
    if let Some(line) = blow.swing.chipped.clone() {
        world.resource_mut::<GameLog>().add(line);
        return;
    }
    let attacker_name = entity_name(world, blow.attacker);
    let target_name = entity_name(world, blow.target);
    let target_is_player = blow.target_is_player;
    let revealed = blow.outcome.lethal
        && world
            .get::<crate::effects::FaerieOnDeath>(blow.target)
            .is_some();
    let outcome = match revealed {
        true => &Outcome {
            lethal: false,
            vorpal: false,
            garrote: false,
        },
        false => &blow.outcome,
    };
    let attacker_unseen = target_is_player && world.get::<Hidden>(blow.attacker).is_some();

    if blow.attacker_is_player {
        let target_is_own_ghost = world.get::<GhostOfPlayer>(blow.target).is_some();
        let mut log = world.resource_mut::<GameLog>();
        return report_player_hit(
            &mut log,
            &target_name,
            &blow.swing,
            outcome,
            target_is_own_ghost,
        );
    }

    if target_is_player && world.get::<GhostOfPlayer>(blow.attacker).is_some() {
        let mut log = world.resource_mut::<GameLog>();
        return report_ghost_self_hit(&mut log, blow.swing.damage, blow.outcome.lethal);
    }

    let target_label = if target_is_player {
        strings::pronoun_you().to_string()
    } else {
        strings::the(&target_name)
    };
    let atk = if attacker_unseen {
        strings::pronoun_something().to_string()
    } else {
        strings::capital_the(&attacker_name)
    };
    let mut log = world.resource_mut::<GameLog>();
    report_monster_hit(
        &mut log,
        &atk,
        &target_label,
        &target_name,
        blow.swing.damage,
        outcome.lethal,
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
        let attacker_name = entity_name(world, blow.attacker);
        silence_shake(world);
        blank_player_glyph(world, blow.target);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = strings::slain_by(&attacker_name);
        return;
    }
    release_biters_grip(world, blow.target, blow.attacker);
    let species = crate::monsters::species_of(world, blow.target);
    if !reveal_faerie(world, blow.target) {
        match world.get::<Helper>(blow.target).is_some() {
            true => crate::companion::mourn(world, blow.target),
            false => pay_for_the_corpse(world, blow.target),
        }
        leave_gear_behind(world, blow.target);
        burst_on_death(world, blow.target);
        crate::spirits::poof(world, blow.target);
    }
    crate::monsters::maybe_shapeshift(world, blow.attacker, species);
}

/// A creature marked [`ExplodesOnDeath`](crate::effects::ExplodesOnDeath) (a
/// rune of justice) goes off as it falls: one fire blast of
/// [`BLAST_RADIUS`](crate::constants::wands::BLAST_RADIUS) where it stood, for
/// one roll of its own power die. It hurts whatever it reaches, the reader
/// included, and a neighbour it kills may burst in turn.
///
/// Called from both ways a monster is finished, [`settle_the_dead`] (a blade)
/// and [`finish_indirect_kill`] (everything else), after its gear is down and
/// before it is gone.
fn burst_on_death(world: &mut World, entity: Entity) {
    if world
        .get::<crate::effects::ExplodesOnDeath>(entity)
        .is_none()
    {
        return;
    }
    crate::effects::revoke(
        world,
        entity,
        Grant::of::<crate::effects::ExplodesOnDeath>(),
    );
    let Some(at) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(power) = world.get::<Fighter>(entity).map(|f| f.power) else {
        return;
    };
    let damage = crate::helpers::roll_dice(world, 1, power.max(1));
    let fighter = world.entity_mut(entity).take::<Fighter>();
    crate::items::elemental_blast(
        world,
        None,
        at,
        crate::constants::wands::BLAST_RADIUS,
        damage,
        Some(crate::components::Element::Fire),
        crate::particles::BlastPalette::Fire,
    );
    if let Some(fighter) = fighter {
        world.entity_mut(entity).insert(fighter);
    }
}

/// A creature that is a faerie shapeshifter underneath ([`FaerieOnDeath`], the
/// dog) does not die: it is revealed as one, and gone. It leaves no corpse,
/// no gore and no score, and says so in pink: "It was never a dog, but a
/// faerie shapeshifter!" That line stands in for the kill line. It drops what it wore, as anything does.
/// `true` when it did, and the caller has nothing left to settle.
fn reveal_faerie(world: &mut World, entity: Entity) -> bool {
    if world.get::<crate::effects::FaerieOnDeath>(entity).is_none() {
        return false;
    }
    let name = entity_name(world, entity);
    world.resource_mut::<GameLog>().add_colored(
        strings::faerie_reveal(crate::identify::article_for(&name), &name),
        LogCategory::Faerie,
    );
    leave_gear_behind(world, entity);
    crate::spirits::poof(world, entity);
    true
}

/// A grip is the biter's, not the floor's: unlike a bear trap's [`Pinned`],
/// which only turns lift, [`Clamped`] lets go the instant whatever clamped
/// you dies — killing the venus flytrap while it has your leg frees you on
/// the same blow, rather than leaving you thrashing against a corpse until
/// the bite's turns happen to run out.
///
/// Gated on [`ClampedBy`] naming `dead` specifically, not just on `dead`
/// having [`Binds`] — two flytraps can share a room, and killing the one that
/// never bit you must not free you from the one that did.
fn release_biters_grip(world: &mut World, dead: Entity, victim: Entity) {
    if world.get::<Binds>(dead).is_some()
        && world
            .get::<ClampedBy>(victim)
            .is_some_and(|by| by.0 == dead)
    {
        revoke(world, victim, Grant::of::<Clamped>());
    }
}

/// Writes the player-attacked-something lines to the log: the hit line (an
/// excellent hit, a glancing scrape, or a plain blow) and, on a kill, the
/// vorpal flourish and the slain line.
/// `target_label` is pre-rendered ("the orc", or "yourself" for a bones
/// ghost sharing the player's own name — see `crate::bones`), so the six
/// string functions below only ever interpolate it, never wrap it in their
/// own "the ".
fn report_player_hit(
    log: &mut GameLog,
    target_name: &str,
    swing: &Swing,
    outcome: &Outcome,
    target_is_own_ghost: bool,
) {
    if target_is_own_ghost {
        match (swing.excellent, swing.glancing) {
            (true, _) => log.add_colored(
                strings::excellent_hit_self(swing.damage),
                LogCategory::Ghost,
            ),
            (_, true) => log.add_colored(strings::glancing_blow_self(), LogCategory::Ghost),
            _ => log.add_colored(strings::plain_hit_self(swing.damage), LogCategory::Ghost),
        }
        match (outcome.lethal, outcome.garrote, outcome.vorpal) {
            (true, true, _) => log.add_colored(strings::garrote_kill_self(), LogCategory::Ghost),
            (true, _, true) => log.add_colored(strings::vorpal_kill_self(), LogCategory::Ghost),
            _ => {}
        }
        if outcome.lethal {
            log.add_colored(strings::you_have_slain_self(), LogCategory::Ghost);
        }
        return;
    }
    match (swing.excellent, swing.glancing) {
        (true, _) => log.add(strings::excellent_hit(target_name, swing.damage)),
        (_, true) => log.add(strings::glancing_blow(target_name)),
        _ => log.add(strings::plain_hit(target_name, swing.damage)),
    }
    match (outcome.lethal, outcome.garrote, outcome.vorpal) {
        (true, true, _) => log.add(strings::garrote_kill(target_name)),
        (true, _, true) => log.add(strings::vorpal_kill(target_name)),
        _ => {}
    }
    if outcome.lethal {
        log.add(strings::you_have_slain(target_name));
    }
}

/// The one direction a pre-rendered label can't fix: a bones ghost sharing
/// the player's own name, attacking the real player. `mob_hits`/`mob_misses`/
/// `mob_strikes_you_down` are written for a third-person subject ("The orc
/// hits you"), which breaks the moment the subject is also "you" — so this
/// gets its own three lines instead of reusing them.
fn report_ghost_self_hit(log: &mut GameLog, damage: i32, lethal: bool) {
    match damage {
        0 => log.add_colored(strings::ghost_self_misses(), LogCategory::Ghost),
        _ => log.add_colored(strings::ghost_self_hits(damage), LogCategory::Ghost),
    }
    if lethal {
        log.add_colored(strings::ghost_self_strikes_you_down(), LogCategory::Ghost);
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
        0 => log.add(strings::mob_misses(atk, target_label)),
        _ => log.add(strings::mob_hits(atk, target_label, damage)),
    }
    if lethal && target_is_player {
        log.add(strings::mob_strikes_you_down(atk));
    }
    if lethal && !target_is_player {
        log.add(strings::mob_kills(atk, target_name));
    }
}
