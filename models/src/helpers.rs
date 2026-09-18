//! Small, item-and-system-agnostic utilities that several modules reach for.
//!
//! Nothing here owns any game rule — these are the plumbing the rules are built
//! from. If a function encodes a decision about how *nihilurk* plays (what a wand
//! does, how loot is rolled), it belongs in the module that owns that decision,
//! not here. What lives here instead:
//!
//! * **Geometry** — [`get_line`] (Bresenham between two tiles).
//! * **Spatial queries** — [`get_entities_at_position`], [`monster_at`],
//!   [`actor_at`], [`tile_of`], [`hostiles_in_view`], [`free_adjacent_tile`],
//!   [`player_sees`].
//! * **Dice** — [`roll_dice`] (`NdM` summed off the shared [`GameRng`]).
//! * **Naming** — [`item_label`] (an entity's display name, or a vague noun)
//!   and [`actor_line`] (the line an effect prints about whoever used it).
//! * **Damage** — [`apply_damage`], and [`took_damage`], which is everything
//!   that happens to a creature *because* it was hurt.
//! * **Defence maths** — [`total_armor_plus`].
//! * **Cosmetics on an entity** — [`spark_burst_at`], [`mark_conditions`],
//!   [`spill_blood`], [`death_burst`], [`leave_smoke`].
//!
//! Player conditions used to live here too; they are their own vocabulary now,
//! in [`crate::conditions`].

use bevy_ecs::{
    entity::Entity,
    prelude::{Or, With},
    world::World,
};
use rand::Rng;
use std::collections::HashSet;

use crossterm::style::Color;

use crate::effects::{ArmorBonus, equipped_total};
use crate::map::{BloodStains, Corpses, FxRng, GameRng, Map, Smoke};
use crate::particles::Particles;
use crate::shake::{ShakeKind, kick_shake};
use crate::{Blood, Faction, Fighter, GameLog, Mob, Name, Player, Position, Renderable};

/// Every tile a straight line from `start` to `end` passes through, endpoints
/// included, in order. Plain integer Bresenham — the line a bolt, a beam, a
/// thrown dagger or a line-of-sight check follows.
pub fn get_line(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();

    // Work in i32 so the deltas can go negative without underflowing.
    let mut x0 = start.x as i32;
    let mut y0 = start.y as i32;
    let x1 = end.x as i32;
    let y1 = end.y as i32;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        // Back to u16 on the way into a Position.
        points.push(Position {
            x: x0 as u16,
            y: y0 as u16,
        });

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
    points
}

/// The defender's "armour plus": the flat `armor_bonus` on its [`Fighter`] plus
/// every [`ArmorBonus`] its equipped gear contributes. This is the *only* part
/// of a target's defence that a trap's — or a hurled weapon's — damage is
/// measured against; the armour *die* is never rolled for either.
pub fn total_armor_plus(world: &World, entity: Entity) -> i32 {
    let base = world
        .get::<Fighter>(entity)
        .map(|f| f.armor_bonus)
        .unwrap_or(0);
    base + equipped_total::<ArmorBonus>(world, entity)
}

/// Every entity — creature, item, feature — standing on `pos`.
pub fn get_entities_at_position(world: &mut World, pos: Position) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &Position)>();
    query
        .iter(world)
        .filter(|(_, p)| **p == pos)
        .map(|(e, _)| e)
        .collect()
}

/// The hostile monster standing on `pos`, if any. Skips the player, allied
/// creatures and anything that isn't a [`Mob`].
pub fn monster_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .find(|(_, p, f)| p.x == pos.x && p.y == pos.y && **f == Faction::Monster)
        .map(|(e, _, _)| e)
}

/// The [`Mob`] standing on `pos`, if any — unlike [`monster_at`], not
/// filtered to [`Faction::Monster`]. The plain tile lookup for anything that
/// only cares whether *something* with a `Mob` is there: [`try_lunge`](crate::combat::try_lunge)'s
/// geometry, [`try_whirl_attack`](crate::combat::try_whirl_attack)'s target, a plain step's attack check.
pub fn mob_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position), With<Mob>>()
        .iter(world)
        .find(|&(_, &p)| p == pos)
        .map(|(e, _)| e)
}

/// The Chebyshev (chessboard) distance between two tiles — "closest diagonal
/// counts as one step" — the adjacency measure used everywhere in combat and
/// AI: a lunge's or whirl's geometry, a chase's next step.
pub fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x as i32 - b.x as i32)
        .abs()
        .max((a.y as i32 - b.y as i32).abs())
}

/// The creature standing on `pos` — anything that acts, friend or foe — never
/// counting `except` (the thrower whose own tile an item is leaving, say).
pub fn actor_at(world: &mut World, pos: Position, except: Entity) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position), Or<(With<Mob>, With<Player>)>>()
        .iter(world)
        .find(|(e, p)| *e != except && **p == pos)
        .map(|(e, _)| e)
}

/// Every [`Mob`] on one of the eight tiles around `center`, `exclude` aside —
/// the battle axe's cleave, swung the moment its wielder's own attack lands.
pub fn adjacent_mobs(world: &mut World, center: Position, exclude: Entity) -> Vec<Entity> {
    let mut q = world.query_filtered::<(Entity, &Position), With<Mob>>();
    q.iter(world)
        .filter(|(e, p)| {
            *e != exclude
                && (p.x, p.y) != (center.x, center.y)
                && (p.x as i32 - center.x as i32).abs() <= 1
                && (p.y as i32 - center.y as i32).abs() <= 1
        })
        .map(|(e, _)| e)
        .collect()
}

/// Where `entity` is standing, as a plain tile pair — what the animation layer
/// and the map overlays want, as against the [`Position`] component itself.
/// `None` for anything with no place in the world.
pub fn tile_of(world: &World, entity: Entity) -> Option<(u16, u16)> {
    world.get::<Position>(entity).map(|p| (p.x, p.y))
}

/// Every hostile standing on a tile `watcher` can currently see. The reach of
/// anything that goes off across a room rather than along a line — the three
/// scrolls that catch a roomful, and whatever else comes to want it.
///
/// A watcher with no viewshed of its own sees nothing: a monster that reads a
/// room-wide scroll aloud is shouting into a room it has no eyes for.
pub fn hostiles_in_view(world: &mut World, watcher: Entity) -> Vec<Entity> {
    let seen: HashSet<(u16, u16)> = world
        .get::<crate::Viewshed>(watcher)
        .map(|v| v.visible_tiles.iter().copied().collect())
        .unwrap_or_default();
    world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .filter(|(_, p, f)| **f == Faction::Monster && seen.contains(&(p.x, p.y)))
        .map(|(e, _, _)| e)
        .collect()
}

/// Throws sparks off `entity`'s own tile ([`Particles::spark_burst`]) — the
/// flourish for magic that lands on a creature or on what it is holding, rather
/// than out in the room. A no-op in a headless world with no effect layer.
pub(crate) fn spark_burst_at(world: &mut World, entity: Entity, color: Color) {
    let Some((x, y)) = tile_of(world, entity) else {
        return;
    };
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.spark_burst(x, y, color);
    }
}

/// Flashes one condition's glyph over each of `caught`
/// ([`Particles::condition_mark`]), a beat apart, so an effect that took a
/// roomful reads as a wave crossing the room rather than every tile blinking at
/// once.
pub(crate) fn mark_conditions(world: &mut World, caught: &[Entity], glyph: char, color: Color) {
    let tiles: Vec<(u16, u16)> = caught.iter().filter_map(|&e| tile_of(world, e)).collect();
    let Some(mut fx) = world.get_resource_mut::<Particles>() else {
        return;
    };
    for (i, &(x, y)) in tiles.iter().enumerate() {
        fx.condition_mark(x, y, glyph, color, i as f32 * 45.0);
    }
}

/// A walkable tile next to `origin` that no entity is standing on, chosen at
/// random. `None` if `origin` is boxed in. Used to place a conjured monster, or
/// to land a creature dragged to the zapper's side.
pub fn free_adjacent_tile(world: &mut World, origin: Position) -> Option<(u16, u16)> {
    let occupied: HashSet<(u16, u16)> = world
        .query::<&Position>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .collect();
    let opts: Vec<(u16, u16)> = {
        let map = world.resource::<Map>();
        let mut v = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (origin.x as i32 + dx, origin.y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if !map.blocks(nx, ny) && !occupied.contains(&(nx, ny)) {
                    v.push((nx, ny));
                }
            }
        }
        v
    };
    if opts.is_empty() {
        return None;
    }
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..opts.len());
    Some(opts[idx])
}

/// Rolls `count` dice of `sides` each and sums them — `count d sides`, off the
/// shared [`GameRng`] so the result is part of the seeded run. A non-positive
/// `count` rolls nothing and sums to 0.
pub fn roll_dice(world: &mut World, count: i32, sides: i32) -> i32 {
    let mut rng = world.resource_mut::<GameRng>();
    (0..count.max(0)).map(|_| rng.0.gen_range(1..=sides)).sum()
}

/// An entity's display name, or a vague fallback so a log line never prints a
/// raw id. Works on anything with a [`Name`] — a potion, a monster, the player's
/// own corpse.
pub fn item_label(world: &World, item: Entity) -> String {
    world
        .get::<Name>(item)
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "item".to_string())
}

/// The line an item's effect prints about whoever used it: `player_line`
/// verbatim when the player did, "The kobold `mob_verb`." when anything else
/// did.
///
/// Every consumable in the game can end up in somebody else's hands — a potion
/// shatters over a monster and is drunk by it, a scroll that lands on something
/// literate is read aloud — so no effect can assume it is talking to the player,
/// and this is the one place that split is written down.
pub(crate) fn actor_line(
    world: &World,
    actor: Entity,
    player_line: &str,
    mob_verb: &str,
) -> String {
    if world.get::<Player>(actor).is_some() {
        return player_line.to_string();
    }
    format!("The {} {mob_verb}.", item_label(world, actor))
}

/// Applies `amount` damage to `entity`'s [`Fighter`] (no-op if it has none), and
/// stains the floor if it bleeds. Death is not handled here — a later system
/// reaps anything that dropped to zero HP.
pub fn apply_damage(world: &mut World, entity: Entity, amount: i32) {
    let hp_before = world.get::<Fighter>(entity).map(|f| f.hp);
    if let Some(mut fighter) = world.get_mut::<Fighter>(entity) {
        fighter.hp -= amount;
    }
    if amount > 0 {
        spill_blood(world, entity, amount, false);
        took_damage(world, entity, hp_before);
    }
}

/// Everything that happens to a creature *because it was hurt*, whatever hurt
/// it: a promise the dungeon made it is off ([`crate::items::break_promises`]),
/// and the player gets the low-HP warning if this blow crossed the line
/// ([`warn_if_newly_low`]).
///
/// Called from the two places damage is dealt — here and
/// [`crate::combat::resolve_attack`], which applies its own. Anything that
/// should notice a wound goes in here rather than in one of them.
pub(crate) fn took_damage(world: &mut World, entity: Entity, hp_before: Option<i32>) {
    crate::items::break_promises(world, entity);
    warn_if_newly_low(world, entity, hp_before);
    crate::monsters::maybe_split(world, entity);
}

/// Logs a one-time "badly wounded" warning as the player's HP crosses down
/// through [`crate::constants::player::LOW_HP_WARNING_FRACTION`] of max — never
/// for a monster, and it only fires on the transition, so it won't repeat
/// every hit while they stay down there. Healing back up and getting hurt low
/// again fires it afresh, which is the point.
///
/// The same transition kicks the long screen shake. Deliberately *this* moment
/// and not "HP is low": the shake is the room reeling as the floor drops out
/// from under you, so it belongs to the crossing, and tying it to the warning
/// means the two can never disagree about when that was.
///
/// Reachable from outside this module because melee does not run through
/// [`apply_damage`] — [`crate::combat::resolve_attack`] applies its own damage
/// and calls this directly.
pub(crate) fn warn_if_newly_low(world: &mut World, entity: Entity, hp_before: Option<i32>) {
    if world.get::<Player>(entity).is_none() {
        return;
    }
    let Some(hp_before) = hp_before else {
        return;
    };
    let Some(fighter) = world.get::<Fighter>(entity) else {
        return;
    };
    let (hp_after, max_hp) = (fighter.hp, fighter.max_hp);
    if hp_after <= 0 {
        return; // dying, not "wounded" — the reaper handles this
    }
    let threshold = (max_hp as f32 * crate::constants::player::LOW_HP_WARNING_FRACTION) as i32;
    if hp_before <= threshold || hp_after > threshold {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add("You are badly wounded!".to_string());
    kick_shake(world, ShakeKind::Wounded);
}

/// Whether the player's viewshed currently covers `(x, y)`.
///
/// The gate on anything cosmetic that would otherwise leak information: a
/// screen shake for a blast in a room you have never seen tells you a blast
/// went off in a room you have never seen.
pub(crate) fn player_sees(world: &mut World, x: u16, y: u16) -> bool {
    world
        .query_filtered::<&crate::Viewshed, With<Player>>()
        .iter(world)
        .next()
        .is_some_and(|v| v.visible_tiles.iter().any(|&(vx, vy)| vx == x && vy == y))
}

const DIRS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// If `entity` bleeds (has [`Blood`]), stain the tile it is standing on and,
/// depending on how hard it was hit, splatter blood onto nearby tiles. Purely
/// cosmetic; call this whenever a creature takes damage.
///
/// `damage` is the HP actually lost and `glancing` marks a chip-damage-only hit.
/// A glancing blow never splatters; otherwise both the number of droplets and
/// how far they can fly scale with the damage dealt.
pub fn spill_blood(world: &mut World, entity: Entity, damage: i32, glancing: bool) {
    if world.get::<Blood>(entity).is_none() {
        return;
    }
    let Some(&pos) = world.get::<Position>(entity) else {
        return;
    };

    // Bail before touching the animation RNG stream if blood is switched off.
    if !world.resource::<BloodStains>().enabled {
        return;
    }

    // Droplet count and reach both grow with the wound — a further 25% heavier
    // than a bare damage/4 would give. A glancing blow only wets the tile
    // underfoot.
    let (droplets, max_reach) = if glancing {
        (0, 0)
    } else {
        (
            (damage * 5 / 16).clamp(0, 10),
            (1 + damage * 5 / 32).clamp(1, 5),
        )
    };

    let splats: Vec<(i32, i32)> = {
        let mut rng = world.resource_mut::<FxRng>();
        (0..droplets)
            .map(|_| {
                let (dx, dy) = DIRS[rng.0.gen_range(0..DIRS.len())];
                let reach = rng.0.gen_range(1..=max_reach);
                (dx * reach, dy * reach)
            })
            .collect()
    };

    world.resource_mut::<BloodStains>().stain(pos.x, pos.y);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.blood_hit(pos.x, pos.y, 0.0);
    }
    if splats.is_empty() {
        return;
    }

    // Blood can't fly through a wall: each droplet streaks along its rolled
    // line and splatters on the first wall it meets instead of wherever the
    // roll aimed it — the streak animation and the hit flash both land there,
    // whether that's open floor or a wall.
    let map = world.resource::<Map>().clone();
    for (dx, dy) in splats {
        let tx = pos.x as i32 + dx;
        let ty = pos.y as i32 + dy;
        if tx < 0 || ty < 0 {
            continue;
        }
        let target = Position {
            x: tx as u16,
            y: ty as u16,
        };
        let mut path: Vec<Position> = Vec::new();
        for step in get_line(pos, target) {
            if step != pos {
                path.push(step);
            }
            if map.blocks(step.x, step.y) {
                break;
            }
        }
        let Some(&landing) = path.last() else {
            continue;
        };
        if !map.blocks(landing.x, landing.y) {
            world
                .resource_mut::<BloodStains>()
                .stain(landing.x, landing.y);
        }

        let pts: Vec<(u16, u16)> = path.iter().map(|p| (p.x, p.y)).collect();
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            let flight_ms = fx.blood_streak(&pts);
            fx.blood_hit(landing.x, landing.y, flight_ms);
        }
    }
}

/// How many turns the puff marking a vanishing lingers — shorter than a fire
/// blast's [`crate::constants::wands::SMOKE_LINGER_TURNS`], since it marks a
/// spot rather than being a fire still smouldering on it.
pub(crate) const VANISHING_SMOKE_TURNS: u8 = 2;

/// Poofs smoke at `pos` — the calling card of a creature that is suddenly not
/// there any more. A teleport's departure, a polymorph, a body dropping through
/// a trapdoor: whatever left, this marks where it stood. Lays a short puff in
/// the persistent [`Smoke`] overlay alongside the instant [`Particles::poof`]
/// flash, so the spot keeps smouldering a couple of turns after the animation
/// has finished.
pub(crate) fn leave_smoke(world: &mut World, pos: Position) {
    leave_tinted_smoke(world, pos, Color::Grey);
}

/// [`leave_smoke`] in a colour: the lingering overlay is the same grey smoke,
/// but the instant puff carries `color`, so a creature yanked away by magic can
/// read differently from one that simply fell.
pub(crate) fn leave_tinted_smoke(world: &mut World, pos: Position, color: Color) {
    world
        .resource_mut::<Smoke>()
        .puff(pos.x, pos.y, VANISHING_SMOKE_TURNS);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.tinted_poof(pos.x, pos.y, 0.0, color);
    }
}

/// The glyphs a death burst's bone shrapnel picks from — angled shards so a
/// burst reads as varied fragments, not one symbol repeated.
const BONE_GLYPHS: [char; 4] = ['/', '\\', '|', '¡'];

/// How many bone shards a death burst throws.
const BONE_SHARD_COUNT: usize = 4;

/// A dying creature's Mortal-Kombat-style flourish, played once right when a
/// hit is determined lethal and *before* the entity is despawned (it still
/// needs the corpse's [`Position`], [`Renderable`] and [`Blood`]): the corpse
/// (`%`) is flung away from the blow that killed it, bone shrapnel scatters
/// outward in every direction, and — if the corpse slams into a wall and the
/// creature bled — the impact splatters blood there too. Purely cosmetic.
///
/// `source` is the attacker's position, when there was one (a melee or thrown
/// hit) — the corpse flies away from it, continuing the line the blow came
/// in on. `None` (an indirect kill: a wand bolt, a fire blast) picks a random
/// direction instead.
///
/// When [`BloodStains`] is disabled (`-nb`), the whole animation — and the RNG
/// it would consume — is skipped, same as [`spill_blood`] going quiet under
/// the flag: the creature simply becomes a corpse where it stood.
pub fn death_burst(world: &mut World, entity: Entity, source: Option<Position>) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };

    // Bail before touching the animation RNG stream if blood is switched off
    // — the creature still leaves a corpse, just with no animation to get
    // there.
    if !world.resource::<BloodStains>().enabled {
        world.resource_mut::<Corpses>().mark(pos.x, pos.y);
        return;
    }

    let has_blood = world.get::<Blood>(entity).is_some();
    let color = world
        .get::<Renderable>(entity)
        .map(|r| r.color)
        .unwrap_or(Color::White);

    let (dx, dy) = match source {
        Some(src) if src != pos => (
            (pos.x as i32 - src.x as i32).signum(),
            (pos.y as i32 - src.y as i32).signum(),
        ),
        _ => {
            let mut rng = world.resource_mut::<FxRng>();
            DIRS[rng.0.gen_range(0..DIRS.len())]
        }
    };
    let reach = world.resource_mut::<FxRng>().0.gen_range(2..=4);

    let map = world.resource::<Map>().clone();
    let target = Position {
        x: (pos.x as i32 + dx * reach).max(0) as u16,
        y: (pos.y as i32 + dy * reach).max(0) as u16,
    };
    let mut path: Vec<Position> = Vec::new();
    let mut hit_wall = false;
    for step in get_line(pos, target) {
        if step != pos {
            path.push(step);
        }
        if map.blocks(step.x, step.y) {
            hit_wall = true;
            break;
        }
    }
    let Some(&landing) = path.last() else {
        return;
    };

    // The corpse itself always rests on open floor — if the flight ended on a
    // wall, walk back along the path to the last passable tile.
    let corpse_tile = if hit_wall {
        path.iter()
            .rev()
            .find(|p| !map.blocks(p.x, p.y))
            .copied()
            .unwrap_or(pos)
    } else {
        landing
    };
    world
        .resource_mut::<Corpses>()
        .mark(corpse_tile.x, corpse_tile.y);

    let pts: Vec<(u16, u16)> = path.iter().map(|p| (p.x, p.y)).collect();
    let flight_ms = world
        .get_resource_mut::<Particles>()
        .map(|mut fx| fx.death_fling(&pts, color))
        .unwrap_or(0.0);

    if hit_wall && has_blood {
        world
            .resource_mut::<BloodStains>()
            .stain(corpse_tile.x, corpse_tile.y);
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.blood_hit(landing.x, landing.y, flight_ms);
        }
    }

    // Bone shrapnel: a handful of shards scattering outward from the death
    // tile, each along its own random line and stopped at the first wall.
    let shard_dirs: Vec<(i32, i32)> = {
        let mut rng = world.resource_mut::<FxRng>();
        (0..BONE_SHARD_COUNT)
            .map(|_| DIRS[rng.0.gen_range(0..DIRS.len())])
            .collect()
    };
    for (i, (sdx, sdy)) in shard_dirs.into_iter().enumerate() {
        let shard_reach = world.resource_mut::<FxRng>().0.gen_range(1..=3);
        let starget = Position {
            x: (pos.x as i32 + sdx * shard_reach).max(0) as u16,
            y: (pos.y as i32 + sdy * shard_reach).max(0) as u16,
        };
        let mut spath: Vec<Position> = Vec::new();
        for step in get_line(pos, starget) {
            if step != pos {
                spath.push(step);
            }
            if map.blocks(step.x, step.y) {
                break;
            }
        }
        if spath.is_empty() {
            continue;
        }
        let glyph = BONE_GLYPHS[i % BONE_GLYPHS.len()];
        let spts: Vec<(u16, u16)> = spath.iter().map(|p| (p.x, p.y)).collect();
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.bone_shard(&spts, glyph);
        }
    }
}
