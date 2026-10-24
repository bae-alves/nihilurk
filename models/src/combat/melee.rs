//! The opposed roll. `resolve_attack` and `melee_attack` fold both fighters into a
//! `Matchup`, roll a `Swing`, land it as an `Outcome`, and pass the `Landed` blow to
//! `report` and `death`. `mean_blow` reads the same numbers without rolling.

use super::*;

/// Resolves a single opposed-roll attack of `attacker` against `target`.
///
/// Damage is `(1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus)`: the
/// attacker's and defender's roll totals are computed independently and then
/// subtracted. Each `1d` is two dice averaged (`roll_die_bell`), excellent hits
/// excepted. Every equipped source of a [`crate::effects::Modifier`] folds
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
    let mut swing = clamp_swing(world, attacker, target, &matchup, swing);
    if !matchup.attacker_is_player && world.get::<crate::effects::Vuln>(target).is_some() {
        swing.damage += (swing.damage * VULN_DAMAGE_PERCENT + 50) / 100;
    }
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
    shatter_armor(world, &blow);
    settle_the_dead(world, &blow);
}

/// The hero's excellent hit breaks the suit a foe has on, with a clink of
/// sparks. A cursed suit goes out harder: splinters of evil magic fly into
/// every creature within [`CURSED_SPLINTER_RADIUS`] that fights the hero, for
/// `1d`[`CURSED_SPLINTER_DIE`] each. The hero, an ally and the wearer, whom
/// the blow already found, are spared.
///
/// Runs before [`settle_the_dead`], so a killing blow breaks the suit instead
/// of leaving it on the corpse.
pub(super) fn shatter_armor(world: &mut World, blow: &Landed) {
    let enemy = world
        .get::<Faction>(blow.target)
        .is_some_and(|&f| hostile(world, f, Faction::Player));
    if !blow.swing.excellent || !blow.attacker_is_player || !enemy {
        return;
    }
    let Some(armor) = equipped_in(world, blow.target, Slot::Body) else {
        return;
    };
    let cursed = world.get::<Curse>(armor).is_some();
    let target_name = entity_name(world, blow.target);
    destroy_worn(world, blow.target, &[armor]);
    world
        .resource_mut::<GameLog>()
        .add(strings::armor_shatters(&target_name));
    let Some(at) = world.get::<Position>(blow.target).copied() else {
        return;
    };
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.impact_sparks(at.x, at.y, Color::Grey, 0.0);
    }
    if cursed {
        splinter(world, blow.target, at);
    }
}

/// The curse let out of a shattered suit at `at`: every foe of the hero in
/// reach but the `wearer` takes a splinter.
pub(super) fn splinter(world: &mut World, wearer: Entity, at: Position) {
    world
        .resource_mut::<GameLog>()
        .add(strings::cursed_armor_splinters().to_string());
    let victims: Vec<(Entity, Position)> = {
        let mut q = world.query::<(Entity, &Position, &Faction)>();
        q.iter(world)
            .filter(|&(e, &p, &f)| {
                e != wearer
                    && chebyshev(p, at) <= CURSED_SPLINTER_RADIUS
                    && hostile(world, f, Faction::Player)
            })
            .map(|(e, &p, _)| (e, p))
            .collect()
    };
    for (victim, p) in victims {
        let cells: Vec<(u16, u16)> = get_line(at, p).iter().map(|c| (c.x, c.y)).collect();
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hurl(&cells, '\'', Color::DarkMagenta);
        }
        let damage = roll_dice(world, 1, CURSED_SPLINTER_DIE);
        let line = strings::splinter_hits(&entity_name(world, victim), damage);
        apply_hit(world, victim, Hit::magic(damage), Some(&line));
    }
}

/// One player melee attack, tricks and all: the plain opposed-roll swing,
/// plus whatever a wielded weapon lends on top of it — an estoc's second
/// strike ([`Fencer`]), a battle axe's cleave ([`crate::effects::Cleaves`]).
/// Both self-check the marker they answer to, so every caller — a walk into a
/// monster's tile, the chain-sickle's free swing — reaches for this and
/// nothing else, exactly the way nothing outside `crate::equipment` has to
/// know a ring exists.
pub fn melee_attack(world: &mut World, attacker: Entity, target: Entity) {
    let is_player = world.get::<Player>(attacker).is_some();
    let attacks = match count_of(world, attacker, Grant::of::<Fencer>()) {
        Some(n) if is_player => n.max(1),
        _ => 1,
    };
    for _ in 0..attacks {
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
pub(super) struct Matchup {
    pub(super) power: i32,
    pub(super) power_bonus: i32,
    pub(super) armor: i32,
    pub(super) armor_bonus: i32,
    /// The strictest ceiling the attacker's gear imposes, if any — a bow.
    /// Folded here rather than looked up again in [`clamp_swing`], because it
    /// comes off the same pass the four numbers above do.
    pub(super) melee_cap: Option<i32>,
    /// Two rules apply only to the hero's own swing: the excellent hit and the
    /// chip-damage floor. Carried here so the three stages below each ask once.
    pub(super) attacker_is_player: bool,
    /// Every blow in this exchange is excellent, whoever swings: the attacker
    /// holds [`crate::effects::Crit`] or the target holds
    /// [`crate::effects::Oof`] (a deck's Chariot, either way up).
    pub(super) always_excellent: bool,
}

/// A blow that has already happened: who swung at whom, what the dice said,
/// and what it did to the creature on the end of it.
///
/// The three aftermath stages — [`punctuate`], [`report_blow`],
/// [`settle_the_dead`] — each need all of it. This is their shared argument
/// list, named, so adding to it is a field rather than a fresh parameter
/// threaded through three signatures.
pub(super) struct Landed {
    pub(super) attacker: Entity,
    pub(super) target: Entity,
    /// Read once in [`resolve_attack`], never re-read. Which side is the
    /// player decides the log's voice, the chip floor and who shakes the
    /// screen, so all three stages want it and none should ask again.
    pub(super) attacker_is_player: bool,
    pub(super) target_is_player: bool,
    pub(super) swing: Swing,
    pub(super) outcome: Outcome,
}

/// One exchange of dice, and what kind of blow it turned out to be.
pub(super) struct Swing {
    pub(super) damage: i32,
    /// The hero's `Nd[power]` crit. Never reported as having done nothing.
    pub(super) excellent: bool,
    /// The armour ate the whole roll and the chip floor is all that got
    /// through. Draws blood, cannot be the killing blow, arms no shake.
    pub(super) glancing: bool,
    /// Set when the target was stone and the blow landed on it: the one line
    /// this blow is reported with, in place of the hit line
    /// ([`crate::effects::stone_chip`]).
    pub(super) chipped: Option<String>,
}

/// What the blow did to the creature on the end of it.
pub(super) struct Outcome {
    pub(super) lethal: bool,
    /// A vorpalized weapon found its bane, or a garrote found a helpless
    /// throat (`garrote`). Either way, skips the HP arithmetic entirely.
    pub(super) vorpal: bool,
    /// The vorpal kill above was specifically the garrote's trick — the log
    /// line and the death flourish read differently from a blade's.
    pub(super) garrote: bool,
}

/// Reads both sides' [`Fighter`] once and folds their gear in.
///
/// An entity with no `Fighter` at all still swings for a bare `1d1` and
/// defends with nothing, which is what lets a test dummy fight without one.
pub(super) fn fold_matchup(world: &World, attacker: Entity, target: Entity) -> Matchup {
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

/// Every value `rolls` dice of `sides` sides can average to (rounded down),
/// with its weight out of `sides.pow(rolls)`. A non-positive die is a flat 0.
pub(super) fn dice_spread(sides: i32, rolls: u32, average: bool) -> Vec<(i32, f64)> {
    if sides <= 0 {
        return vec![(0, 1.0)];
    }
    let mut sums: Vec<(i32, f64)> = vec![(0, 1.0)];
    for _ in 0..rolls {
        let mut next = std::collections::BTreeMap::new();
        for (sum, weight) in &sums {
            for face in 1..=sides {
                *next.entry(sum + face).or_insert(0.0) += weight / f64::from(sides);
            }
        }
        sums = next.into_iter().collect();
    }
    let divisor = if average { rolls as i32 } else { 1 };
    sums.into_iter().map(|(s, w)| (s / divisor, w)).collect()
}

/// The average damage one blow from `attacker` does to `target`, worked out
/// exactly from the same numbers `roll_swing` and `clamp_swing` use: both
/// sides' dice, bonuses and worn gear, the hero's excellent hit and chip
/// floor, and a launcher's melee cap. Abilities, spells, stone and
/// protection are left out. Rolls no dice.
pub fn mean_blow(world: &World, attacker: Entity, target: Entity) -> f64 {
    let m = fold_matchup(world, attacker, target);
    let armor = dice_spread(m.armor, BELL_CURVE_DICE as u32, true);
    let normal = dice_spread(m.power, BELL_CURVE_DICE as u32, true);
    let excellent = dice_spread(m.power, EXCELLENT_HIT_DICE as u32, false);
    let (p_excellent, normal_share) = match (m.always_excellent, m.attacker_is_player) {
        (true, _) => (1.0, 0.0),
        (false, true) => (EXCELLENT_HIT_CHANCE, 1.0 - EXCELLENT_HIT_CHANCE),
        (false, false) => (0.0, 1.0),
    };
    let blow = |attack: i32, defence: i32, is_excellent: bool| {
        let net = attack + m.power_bonus - defence - m.armor_bonus;
        let mut damage = match m.attacker_is_player && !is_excellent && net < CHIP_DAMAGE {
            true => CHIP_DAMAGE,
            false => net.max(0),
        };
        if let Some(cap) = m.melee_cap {
            damage = damage.min(cap);
        }
        if is_excellent {
            damage = damage.max(CHIP_DAMAGE);
        }
        f64::from(damage)
    };
    let mut mean = 0.0;
    for (defence, d_weight) in &armor {
        for (attack, a_weight) in &normal {
            mean += normal_share * a_weight * d_weight * blow(*attack, *defence, false);
        }
        for (attack, a_weight) in &excellent {
            mean += p_excellent * a_weight * d_weight * blow(*attack, *defence, true);
        }
    }
    mean
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
pub(super) fn roll_swing(world: &mut World, matchup: &Matchup) -> Swing {
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
pub(super) fn clamp_swing(
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
pub(super) fn chip_on_stone(
    world: &World,
    attacker: Entity,
    target: Entity,
    swing: &mut Swing,
) -> bool {
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
pub(super) fn land_swing(
    world: &mut World,
    attacker: Entity,
    target: Entity,
    swing: &Swing,
) -> Outcome {
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

    crate::conduct::player_broke(world, attacker, crate::conduct::Conduct::Attacking);
    let hp_before = world.get::<Fighter>(target).map(|f| f.hp);
    let mut lethal = false;
    if let Some(mut fighter) = world.get_mut::<Fighter>(target) {
        fighter.hp -= swing.damage;
        if vorpal {
            fighter.hp = 0;
        }
        lethal = fighter.hp <= 0;
    }
    if lethal && !vorpal && crate::helpers::armor_takes_last_hit(world, target) {
        lethal = false;
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
pub(super) fn garrote_vorpal(world: &World, attacker: Entity, target: Entity) -> bool {
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
