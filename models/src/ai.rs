//! What every monster on the floor does with its turn.
//!
//! [`ai`] is the whole entry point: it snapshots the player once, works out
//! how many monster rounds this player turn buys, then hands both to
//! [`monster_round`], which banks tempo energy and steps every mob in up to
//! two passes so a `Fast` monster can act twice. A single mob's turn is
//! [`step_one_mob`]: the gates (dead, asleep, stone, out of energy), then a
//! percept, a decision and an action. The decision is not made here: it is
//! the mob's rule set's, in [`crate::agents`]. This file keeps the clock, the
//! gates and the hands.
//!
//! Everything here reads the player's view, never their eyes: a blinded
//! player still stands in whatever light the monsters around them can see by,
//! so blindness never doubles as a way to hide.

use crate::agents::{Action, Percept, Sighting, leashed, rule_set_for, think};
use crate::components::*;
use crate::effects::{Asleep, Blind, Clamped, Petrified, Phasing, Pinned, Rooted, Stealthy, Swims};
use crate::helpers::chebyshev;
use crate::map::{MAP_HEIGHT, MAP_WIDTH, Map, TileType, tile_index};
use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`.
//
//   STEALTH_RANGE         how close a stealthy player must be to be noticed
use crate::constants::rings::STEALTH_RANGE;

/// Monster turn. Exclusive so it can move each mob more than once: the player is
/// the clock, and every creature banks [`Speed`] energy each of the player's
/// turns, spending [`Speed::COST`] per step. A `Fast` monster gets two moves per
/// player turn and a `Slow` one moves every other turn. A mob with no [`Speed`]
/// component (test dummies) simply acts once.
///
/// The player's own tempo scales how many monster *rounds* a single turn buys: a
/// `Fast` player's turns alternate one-round / no-round (tracked by
/// [`PlayerTempo::fast_parity`]), a `Quick` player's third turn of every three
/// buys none ([`PlayerTempo::quick_beat`]), and a `Slow` player's one turn
/// buys two rounds.
/// Every other schedule step (traps, visibility, the Dungeon Lord's patience)
/// still ticks exactly once per player turn.
///
/// Assumes [`crate::monsters::reveal_mimics`] has already run this turn, so
/// any [`Mimic`] still on a mob is a xeroc genuinely still disguised, not one
/// merely unprocessed yet — the disguise gate this loop reads for
/// `MovementType::Ambush` depends on that being settled first.
pub fn ai(world: &mut World) {
    // THE WORLD: while the player holds time still, nothing else moves, ally
    // or foe.
    if world
        .query_filtered::<(), (With<Player>, With<crate::effects::TimeStopped>)>()
        .iter(world)
        .next()
        .is_some()
    {
        return;
    }
    // The whole turn is decided against one snapshot of the player, taken
    // before any monster moves — so a mob that steps aside in pass 0 cannot
    // change what the mob after it can see.
    #[allow(clippy::type_complexity)] // one query for the whole player snapshot
    let Some((player_entity, player_pos, visible_tiles, player_blind, player_faction)) = ({
        let mut q = world
            .query_filtered::<(Entity, &Position, &Viewshed, Option<&Blind>, &Faction), With<Player>>(
            );
        q.iter(world).next().map(|(e, p, v, b, f)| {
            let seen: HashSet<(u16, u16)> = v.visible_tiles.iter().copied().collect();
            (e, *p, seen, b.is_some(), *f)
        })
    }) else {
        return;
    };
    // The tempo the player is *acting* at, gear and all — a ring of slow
    // digestion is a slowing like any other, and buys the floor the same extra
    // round a potion of paralysis would.
    let player_speed = crate::conditions::tempo(world, player_entity);
    // A stealthy player is not there as far as the floor is concerned until
    // they are within arm's reach (see [`notices`]).
    let player_stealthy = world.get::<Stealthy>(player_entity).is_some();

    // How many monster rounds this one player turn is worth.
    let mut rounds = match player_speed {
        SpeedKind::Normal => 1,
        SpeedKind::Slow => 2,
        SpeedKind::Fast => {
            // Two player turns per monster round: act on the turn the parity
            // flips back off, skip the other.
            let act = world
                .get_resource_mut::<PlayerTempo>()
                .map(|mut t| {
                    t.fast_parity = !t.fast_parity;
                    !t.fast_parity
                })
                .unwrap_or(true);
            if act { 1 } else { 0 }
        }
        // Half again as fast: two monster rounds bought per three player
        // turns, so every third turn is free. The lurk's tempo.
        SpeedKind::Quick => {
            let beat = world
                .get_resource_mut::<PlayerTempo>()
                .map(|mut t| {
                    t.quick_beat = (t.quick_beat + 1) % 3;
                    t.quick_beat
                })
                .unwrap_or(1);
            if beat == 0 { 0 } else { 1 }
        }
    };
    // A greatclub's heavy swing (`crate::effects::HeavySwing`) costs its
    // wielder a beat of their own the instant it lands — one more monster
    // round, on top of whatever the player's own tempo already bought, spent
    // the moment it's asked for.
    if world
        .get_resource_mut::<ExtraMonsterRound>()
        .is_some_and(|mut r| std::mem::take(&mut r.0))
    {
        rounds += 1;
    }

    let warded = door_ward(world, player_entity, player_pos);
    let map = world.resource::<Map>().clone();

    // Blindness is the player's problem, not the dungeon's. A blinded hero's
    // viewshed is cut to the 3x3 they can feel around them, but the monsters in
    // the lit room they are standing in can all still see them perfectly well —
    // so the AI works off the view the player *would* have with their eyes open.
    // Without this, drinking a potion of blindness would be a way to hide.
    let visible_tiles = match player_blind {
        true => crate::visibility::visible_from(&map, &player_pos, false),
        false => visible_tiles,
    };

    let mut ctx = AiCtx {
        pass: 0,
        player: player_entity,
        player_pos,
        player_faction,
        visible: &visible_tiles,
        stealthy: player_stealthy,
        warded,
        map: &map,
    };
    for _round in 0..rounds {
        monster_round(world, &mut ctx);
    }
}

/// A doorway is a ward while the player's last turn was a step onto it: every
/// hostile that can see them does nothing. Any other turn spent there — a
/// swing, a throw, a spell, an item — cracks the frame for good (the map's
/// inert doorways, drawn grey) and the ward with it.
fn door_ward(world: &mut World, player: Entity, at: Position) -> bool {
    let map = world.resource::<Map>();
    if map.tile(at.x, at.y) != TileType::Door || map.is_inert_door(at.x, at.y) {
        return false;
    }
    if world.get::<EntityMoved>(player).is_some() {
        return true;
    }
    world
        .resource_mut::<Map>()
        .inert_doors
        .insert(tile_index(at.x, at.y));
    world
        .resource_mut::<GameLog>()
        .add(strings::doorway_goes_inert());
    false
}

/// Whether a mob standing on `mob_pos` knows where the player is this turn.
///
/// Ordinarily that is simply "is its tile in the player's view" — sight is
/// symmetrical in nihilurk. A ring of stealth breaks the symmetry: the player can
/// see the length of a lit room and nothing in it can see them back until they
/// are [`STEALTH_RANGE`] tiles away, at which point being quiet stops helping.
fn notices(seen: bool, stealthy: bool, player_pos: Position, mob_pos: Position) -> bool {
    if !seen {
        return false;
    }
    if !stealthy {
        return true;
    }
    let dx = (player_pos.x as i32 - mob_pos.x as i32).abs();
    let dy = (player_pos.y as i32 - mob_pos.y as i32).abs();
    dx.max(dy) <= STEALTH_RANGE
}

/// One full round of monster movement: bank energy, then up to two passes so a
/// `Fast` monster can act twice. A pass that moves nobody ends the round.
/// Everything a mob's turn is decided against that is the same for every mob
/// on the floor: where the player is, what they can see, and what the ground
/// looks like.
///
/// Gathered once per round rather than passed as eight arguments. The
/// argument list is what this replaces — it had grown a `player_stealthy`
/// bool threaded through three functions so that `notices` could ask one
/// question, and the next thing a mob wants to know about the player would
/// have been a ninth.
struct AiCtx<'a> {
    /// Which pass of the round this is. A `Fast` mob acts on both.
    pass: usize,
    player: Entity,
    player_pos: Position,
    player_faction: Faction,
    /// The player's viewshed. A mob acts on what the *player* can see, which
    /// is what keeps the floor quiet out of sight.
    visible: &'a HashSet<(u16, u16)>,
    /// Whether the player is currently hard to notice — a ring of stealth.
    stealthy: bool,
    /// Whether the player stands on an intact doorway they just stepped onto:
    /// every hostile that can see them holds still (see [`door_ward`]).
    warded: bool,
    map: &'a Map,
}

impl AiCtx<'_> {
    /// Whether `mob`, standing at `mob_pos`, has noticed the player.
    fn noticed_by(&self, mob_pos: Position) -> bool {
        let in_view = self.visible.contains(&(mob_pos.x, mob_pos.y));
        notices(in_view, self.stealthy, self.player_pos, mob_pos)
    }
}

fn monster_round(world: &mut World, ctx: &mut AiCtx) {
    // Bank this round's energy for every actor with a tempo, at the tempo it is
    // actually acting at — gear that weighs a creature down banks it less. The
    // pool is capped so a monster left alone off-screen can't hoard a dozen free
    // moves for when it finally reaches you.
    {
        let actors: Vec<Entity> = world
            .query_filtered::<Entity, With<Speed>>()
            .iter(world)
            .collect();
        for actor in actors {
            let rate = crate::conditions::tempo(world, actor).rate();
            if let Some(mut speed) = world.get_mut::<Speed>(actor) {
                speed.energy = (speed.energy + rate).min(2 * Speed::COST);
            }
        }
    }

    for pass in 0..2 {
        ctx.pass = pass;
        let mob_list: Vec<Entity> = world
            .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
            .iter(world)
            .collect();

        // Rebuilt each pass so a mob that moved in pass 0 is seen in its new
        // tile in pass 1.
        let mut spatial = actor_positions(world, ctx.player, ctx.player_pos, ctx.player_faction);

        let mut any_acted = false;
        for mob in mob_list {
            any_acted |= step_one_mob(world, mob, ctx, &mut spatial);
        }
        if !any_acted {
            break;
        }
    }
}

/// The tile → actor map for a pass: the player, plus every mob at its current
/// position.
fn actor_positions(
    world: &mut World,
    player_entity: Entity,
    player_pos: Position,
    player_faction: Faction,
) -> HashMap<(u16, u16), (Entity, Faction)> {
    let mut spatial = HashMap::new();
    spatial.insert(
        (player_pos.x, player_pos.y),
        (player_entity, player_faction),
    );
    let mut q =
        world.query_filtered::<(Entity, &Position, &Faction), (With<Mob>, Without<Player>)>();
    for (e, p, f) in q.iter(world) {
        spatial.insert((p.x, p.y), (e, *f));
    }
    spatial
}

/// One mob's turn within a pass: forfeit if dead, asleep, stone or out of
/// energy; otherwise build its [`Percept`], let its rule set pick an
/// [`Action`] ([`crate::agents::think`]) and carry that out. Returns whether it
/// did anything — a whole idle pass ends the round.
fn step_one_mob(
    world: &mut World,
    mob: Entity,
    ctx: &AiCtx,
    spatial: &mut HashMap<(u16, u16), (Entity, Faction)>,
) -> bool {
    // Already dead forfeits everything. An indirect kill — a spell, a wand bolt
    // — only zeroes the HP and leaves the body for `reaper_system` at the far
    // end of the turn, and `spell_system` resolves before this does, so a
    // corpse is reachable here. It does not get a parting shot. Nor does a
    // mob already gone from the world: the pass listed it before something
    // this turn despawned it (polymorph respawns its target as a new entity).
    if world.get_entity(mob).is_none() || world.get::<Fighter>(mob).is_some_and(|f| f.hp <= 0) {
        return false;
    }
    // Asleep — or stone — forfeits the turn outright. Pinned or rooted means
    // it cannot take a step, but a foe within reach still gets bitten.
    if world.get::<Asleep>(mob).is_some() || world.get::<Petrified>(mob).is_some() {
        return false;
    }
    if !can_afford_step(world, mob, ctx.pass) {
        return false;
    }
    if ctx.warded
        && world.get::<Faction>(mob) == Some(&Faction::Monster)
        && ctx.noticed_by(*world.get::<Position>(mob).unwrap())
    {
        return false;
    }

    let percept = perceive(world, mob, ctx, spatial);
    let movement = world.get::<Mob>(mob).unwrap().movement_type;
    let set = rule_set_for(movement, percept.helper);
    let action = think(&percept, set);
    let leash = leashed(&percept, set);
    let (at, pinned, swims) = (percept.at, percept.pinned, percept.swims);
    drop(percept);
    act(
        world, mob, action, at, leash, pinned, swims, ctx.map, spatial,
    )
}

/// What `mob` knows this turn — see [`Percept`]. Its foes are everything it
/// would come to blows with standing on a tile the player can see (and not
/// hidden from them), nearest first; the player only once it has noticed them.
fn perceive<'a>(
    world: &World,
    mob: Entity,
    ctx: &AiCtx<'a>,
    spatial: &HashMap<(u16, u16), (Entity, Faction)>,
) -> Percept<'a> {
    let at = *world.get::<Position>(mob).unwrap();
    let faction = *world.get::<Faction>(mob).unwrap();
    let noticed = ctx.noticed_by(at);
    // ponytail: one pass over `spatial` per mob per pass; index by faction if
    // floors ever hold hundreds of mobs.
    let mut foes: Vec<Sighting> = spatial
        .iter()
        .filter(|&(tile, &(who, their))| {
            hostile(world, faction, their)
                && ctx.visible.contains(tile)
                && world.get::<Hidden>(who).is_none()
                && (who != ctx.player || noticed)
        })
        .map(|(&(x, y), &(who, _))| Sighting {
            who,
            at: Position { x, y },
            is_player: who == ctx.player,
        })
        .collect();
    // Ties broken on the tile, not on the map's iteration order, which is
    // random.
    foes.sort_by_key(|f| (chebyshev(at, f.at), f.at.y, f.at.x));
    Percept {
        at,
        in_view: ctx.visible.contains(&(at.x, at.y)),
        player_at: ctx.player_pos,
        noticed,
        pinned: world.get::<Pinned>(mob).is_some()
            || world.get::<Rooted>(mob).is_some()
            || world.get::<Clamped>(mob).is_some(),
        swims: world.get::<Swims>(mob).is_some(),
        phasing: world.get::<Phasing>(mob).is_some(),
        launcher: crate::equipment::wielded_launcher(world, mob).is_some(),
        // Only what its Magic can pay for: a dry caster has nothing to cast.
        spellset: world
            .get::<Spellset>(mob)
            .into_iter()
            .flat_map(|s| s.slots.iter().copied())
            .filter(|&spell| crate::items::can_afford_spell(world, mob, spell))
            .collect(),
        // The dungeon's own randomness, not the seed's: which spell a monster
        // tries and which way a confused one lurches have never been part of
        // what a seed replays.
        roll: getrandom::u32().unwrap_or(0),
        helper: world.get::<Helper>(mob).is_some(),
        ally: faction == Faction::Ally,
        aggravated: world
            .get::<Aggravated>(mob)
            .map(|a| Position { x: a.tx, y: a.ty }),
        foes,
        map: ctx.map,
    }
}

/// Carries out what the rule set decided. A strike is a step onto the foe's
/// tile, so both go through the same checks — a strike across a doorway's
/// corner is as impossible as a step there.
#[allow(clippy::too_many_arguments)] // one action, and what the mob is to act on it
fn act(
    world: &mut World,
    mob: Entity,
    action: Action,
    at: Position,
    leashed: bool,
    pinned: bool,
    swims: bool,
    map: &Map,
    spatial: &mut HashMap<(u16, u16), (Entity, Faction)>,
) -> bool {
    let (dx, dy) = match action {
        Action::Wait => return false,
        Action::Shoot(foe) => {
            crate::items::monster_ranged_attack(world, mob, foe);
            spend_energy(world, mob);
            return true;
        }
        Action::Cast(spell, target) => {
            if !crate::items::pay_for_spell(world, mob, spell) {
                return false;
            }
            crate::items::apply_spell_effect(world, mob, target, spell, 1);
            spend_energy(world, mob);
            return true;
        }
        Action::Step(dx, dy) => (dx, dy),
        Action::Strike(foe) => {
            let Some(there) = world.get::<Position>(foe).copied() else {
                return false;
            };
            (there.x as i16 - at.x as i16, there.y as i16 - at.y as i16)
        }
    };
    let new_x = (at.x as i16 + dx) as u16;
    let new_y = (at.y as i16 + dy) as u16;
    // A phasing mob is stopped only by the map's edge.
    let ghost = world.get::<Phasing>(mob).is_some();
    let on_map = new_x < MAP_WIDTH && new_y < MAP_HEIGHT;
    if !(on_map && (ghost || mob_can_enter(map, leashed, at, new_x, new_y, swims))) {
        return false;
    }
    let faction = *world.get::<Faction>(mob).unwrap();

    // Someone in the way: a swing if it is a foe, otherwise stand.
    if let Some(&(target, their)) = spatial.get(&(new_x, new_y)) {
        if !hostile(world, faction, their) {
            return false;
        }
        world
            .resource_mut::<AttackQueue>()
            .attacks
            .push(WantsToAttack {
                attacker: mob,
                target,
            });
        spend_energy(world, mob);
        return true;
    }

    // Held where it stands: it may lash out (above) but not step.
    if pinned {
        return false;
    }

    spatial.remove(&(at.x, at.y));
    if let Some(mut pos) = world.get_mut::<Position>(mob) {
        pos.x = new_x;
        pos.y = new_y;
    }
    spatial.insert((new_x, new_y), (mob, faction));
    world.entity_mut(mob).insert(EntityMoved);
    spend_energy(world, mob);
    true
}

/// Whether `mob` can spend a step this pass: with a tempo it must be able to
/// afford [`Speed::COST`]; without one it acts on pass 0 only.
fn can_afford_step(world: &mut World, mob: Entity, pass: usize) -> bool {
    match world.get::<Speed>(mob).map(|s| s.energy) {
        Some(energy) => energy >= Speed::COST,
        None => pass == 0,
    }
}

/// Whether these two factions come to blows. `Spirits` hinges on
/// [`SpiritsHostile`]: a peaceful spirit fights nobody and nobody fights it;
/// once it flips, spirits fight everyone but each other.
fn hostile(world: &World, a: Faction, b: Faction) -> bool {
    match (a, b) {
        (Faction::Monster, Faction::Player)
        | (Faction::Monster, Faction::Ally)
        | (Faction::Ally, Faction::Monster) => true,
        (Faction::Spirits, Faction::Spirits) => false,
        (Faction::Spirits, _) | (_, Faction::Spirits) => {
            world.get_resource::<SpiritsHostile>().is_some_and(|s| s.0)
        }
        _ => false,
    }
}

/// Whether `mob` may step onto `(new_x, new_y)`: on the map, somewhere its
/// feet can stand (deep water only if it `swims`), a legal diagonal, and —
/// when `leashed` — not out of a room into a corridor or doorway (the room
/// leash, which a rule set asks for: see [`crate::agents::RuleSet::leashed`]).
fn mob_can_enter(
    map: &Map,
    leashed: bool,
    mob_pos: Position,
    new_x: u16,
    new_y: u16,
    swims: bool,
) -> bool {
    if new_x >= MAP_WIDTH || new_y >= MAP_HEIGHT {
        return false;
    }
    if !map.walkable(new_x, new_y, swims) {
        return false;
    }
    if !map.diagonal_step_ok(mob_pos.x, mob_pos.y, new_x, new_y) {
        return false;
    }
    // The leash belongs to the tile a monster is standing on, not to the
    // monster: from room floor a chaser won't step into a corridor or through
    // a doorway, and from a corridor it is tethered to nothing and may cross a
    // door freely. So a corridor monster that steps into a room is leashed
    // from its very next step — which can be the second pass of the same
    // player turn.
    let room_leashed = leashed
        && map.tile(mob_pos.x, mob_pos.y) == TileType::Room
        && matches!(map.tile(new_x, new_y), TileType::Passage | TileType::Door);
    !room_leashed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MAP_TILE_COUNT;
    use fixedbitset::FixedBitSet;

    const OTHERS: [Faction; 3] = [Faction::Player, Faction::Ally, Faction::Monster];

    /// A mob the pass already listed can be gone by its turn: polymorph
    /// despawns its target and spawns the new shape as a fresh entity.
    #[test]
    fn a_mob_despawned_mid_pass_forfeits_its_turn() {
        let mut world = World::new();
        let player = world.spawn(Player).id();
        let mob = world
            .spawn(Mob {
                movement_type: MovementType::Chase,
            })
            .id();
        world.despawn(mob);
        let map = Map {
            tiles: vec![TileType::Room; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        let visible = HashSet::new();
        let ctx = AiCtx {
            pass: 0,
            player,
            player_pos: Position { x: 1, y: 1 },
            player_faction: Faction::Player,
            visible: &visible,
            stealthy: false,
            warded: false,
            map: &map,
        };
        assert!(!step_one_mob(&mut world, mob, &ctx, &mut HashMap::new()));
    }

    #[test]
    fn a_peaceful_spirit_fights_nobody_and_nobody_fights_it() {
        let mut world = World::new();
        world.init_resource::<SpiritsHostile>();
        for other in OTHERS {
            assert!(!hostile(&world, Faction::Spirits, other), "{other:?}");
            assert!(!hostile(&world, other, Faction::Spirits), "{other:?}");
        }
    }

    #[test]
    fn an_angered_spirit_fights_everyone_but_its_own() {
        let mut world = World::new();
        world.insert_resource(SpiritsHostile(true));
        for other in OTHERS {
            assert!(hostile(&world, Faction::Spirits, other), "{other:?}");
            assert!(hostile(&world, other, Faction::Spirits), "{other:?}");
        }
        assert!(!hostile(&world, Faction::Spirits, Faction::Spirits));
    }

    #[test]
    fn room_monsters_are_room_leashed() {
        let mut map = Map {
            tiles: vec![TileType::Wall; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        map.tiles[tile_index(5, 5)] = TileType::Room;
        map.tiles[tile_index(6, 5)] = TileType::Passage;

        assert!(!mob_can_enter(
            &map,
            true,
            Position { x: 5, y: 5 },
            6,
            5,
            false,
        ));
    }

    #[test]
    fn the_leash_follows_the_tile_not_the_monster() {
        let mut map = Map {
            tiles: vec![TileType::Wall; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        map.tiles[tile_index(5, 5)] = TileType::Passage;
        map.tiles[tile_index(6, 5)] = TileType::Room;

        assert!(mob_can_enter(
            &map,
            true,
            Position { x: 5, y: 5 },
            6,
            5,
            false,
        ));
    }

    #[test]
    fn corridor_monsters_can_step_through_a_door() {
        let mut map = Map {
            tiles: vec![TileType::Wall; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        map.tiles[tile_index(5, 5)] = TileType::Passage;
        map.tiles[tile_index(6, 5)] = TileType::Door;

        assert!(mob_can_enter(
            &map,
            true,
            Position { x: 5, y: 5 },
            6,
            5,
            false,
        ));
    }

    #[test]
    fn only_a_swimmer_takes_to_the_water() {
        let mut map = Map {
            tiles: vec![TileType::Wall; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        map.tiles[tile_index(5, 5)] = TileType::Room;
        map.tiles[tile_index(6, 5)] = TileType::Water;

        let steps = |swims| mob_can_enter(&map, true, Position { x: 5, y: 5 }, 6, 5, swims);
        assert!(!steps(false), "a walker stops at the shore");
        assert!(steps(true), "a swimmer goes in");
    }
}

/// Deducts one action's worth of energy from `entity`, if it has a tempo.
fn spend_energy(world: &mut World, entity: Entity) {
    if let Some(mut speed) = world.get_mut::<Speed>(entity) {
        speed.energy -= Speed::COST;
    }
}
