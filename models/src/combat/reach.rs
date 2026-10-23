//! Attacks that cover ground: the lunge, the whirl on a move, and the reach weapon.
//! Each finds its target, then reuses `melee`'s roll and the `report` and `death`
//! aftermath, so damage is still decided in one place.

use super::*;

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
    let Some((near, target)) = lunge_target(world, attacker, dx, dy) else {
        return false;
    };
    resolve_lunge(world, attacker, target);
    if let Some(mut pos) = world.get_mut::<Position>(attacker) {
        *pos = near;
    }
    crate::helpers::mark_moved(world, attacker);
    true
}

/// Where a lunge from `attacker` toward `(dx, dy)` would land, and whom it
/// would strike: `None` unless `attacker` is a player with [`Lunges`], the tile
/// dead ahead is open and empty, and an enemy stands past it. Reads the world
/// and changes nothing.
pub fn lunge_target(
    world: &mut World,
    attacker: Entity,
    dx: i16,
    dy: i16,
) -> Option<(Position, Entity)> {
    if world.get::<Player>(attacker).is_none() || world.get::<Lunges>(attacker).is_none() {
        return None;
    }
    let origin = world.get::<Position>(attacker).copied()?;
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
    let target = near_clear.then(|| monster_at(world, far)).flatten()?;
    Some((near, target))
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
pub(super) fn resolve_lunge(world: &mut World, attacker: Entity, target: Entity) {
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
