//! What a landed blow says and shows: the spark on the tile, the flourish, and the log
//! lines for the player's hits and for everyone else's.

use super::*;

/// The spark a blow leaves on the tile it landed on. Every blow leaves exactly
/// one, which is why this is an enum rather than three flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Spark {
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
pub(super) struct Flourish {
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
pub(super) fn punctuate(world: &mut World, blow: &Landed) {
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
pub(super) fn report_blow(world: &mut World, blow: &Landed) {
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
        let mut log = world.resource_mut::<GameLog>();
        return report_player_hit(&mut log, &target_name, &blow.swing, outcome);
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

/// Writes the player-attacked-something lines to the log: the hit line (an
/// excellent hit, a glancing scrape, or a plain blow) and, on a kill, the
/// vorpal flourish and the slain line.
pub(super) fn report_player_hit(
    log: &mut GameLog,
    target_name: &str,
    swing: &Swing,
    outcome: &Outcome,
) {
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

/// Writes the something-attacked-a-creature lines to the log. `atk` and
/// `target_label` are pre-rendered ("The orc" / "Something", "you" / "the rat")
/// so this never has to know whether the player was on either end.
pub(super) fn report_monster_hit(
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
