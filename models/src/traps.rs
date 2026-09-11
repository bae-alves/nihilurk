//! Dungeon traps, in the spirit of the six classic Rogue traps.
//!
//! A trap is an ordinary ECS entity — never a [`TileType`](crate::map::TileType)
//! and never an [`Item`] — that sits on the floor as a `^` glyph, springs the
//! instant anything with a [`Position`] steps onto its tile, and cannot be
//! picked up. Each trap rolls one of three discovery styles at spawn (equal
//! odds):
//!
//! * [`TrapReveal::Sight`] — shows itself the moment its tile enters the
//!   player's viewshed.
//! * [`TrapReveal::Adjacent`] — stays hidden until the player is standing next
//!   to it.
//! * [`TrapReveal::Triggered`] — invisible until something sets it off.
//!
//! roog has no "wait a turn and search" action, so those three modes are the
//! only ways a trap ever comes to light before it bites.
//!
//! The trap components themselves ([`Trap`], [`Snare`], [`SnareKind`],
//! [`TrapEffect`], [`TrapReveal`], [`EntityMoved`]) are nouns and live in
//! [`crate::components`]; this module is the verbs — the catalog row
//! ([`TrapDef`]), the spring, and the per-effect mechanics.
//!
//! ## Turn wiring
//!
//! Movement code (the player in `move_player`, monsters in [`crate::ai`]) tags
//! the mover with [`EntityMoved`]. [`trap_system`] runs just after the AI, walks
//! that list, and springs any trap sharing a tile with a mover. [`snare_system`]
//! runs at the very top of the turn and ages [`Snare`] (bear trap / sleep gas)
//! down, so the turn a snare is applied is never the turn it is decremented.
//!
//! ## Bear trap
//!
//! A [`SnareKind::Bear`] snare impedes *movement only*. The victim can still
//! strike an adjacent foe; a step, though, becomes a bloody lurch against the
//! jaws — one wasted turn and [`bear_trap_thrash`]. [`SnareKind::Sleep`] is the
//! total one: no action of any kind.
//!
//! ## Armour rule
//!
//! The damage traps (arrow, dart) *ignore the defender's armour die* but still
//! subtract its flat bonus — "armour plus", i.e. `armor_bonus` plus any equipped
//! suit's `arm_bonus` (see [`crate::helpers::total_armor_plus`]).
//!
//! Their bite also scales with depth, in three tiers ending at floors 4, 8 and
//! 13: each tier adds a point to the arrow trap's roll and a point to the dart
//! trap's permanent power drain. The dials are [`crate::constants::traps`].

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::constants::traps::{
    ARROW_DAMAGE_BONUS, ARROW_DAMAGE_DICE, ARROW_DAMAGE_PER_TIER, ARROW_DAMAGE_SIDES,
    BEAR_TRAP_THRASH_DAMAGE, BEAR_TRAP_THRASH_GORE, DART_DAMAGE_DICE, DART_DAMAGE_SIDES,
    DART_POWER_DRAIN_BASE, DART_POWER_DRAIN_PER_TIER, TRAP_DAMAGE_TIER_LAST_DEPTH,
};
use crate::effects::SustainsStrength;
use crate::helpers::{apply_damage, roll_dice, spill_blood, total_armor_plus};
use crate::map::{
    FINAL_DEPTH, GameRng, LevelChange, MAP_HEIGHT, MAP_WIDTH, Map, TileType, tile_index,
    transition_level,
};
use crate::particles::Particles;

impl TrapEffect {
    /// The name shown once the trap is known — read straight off the row.
    pub fn label(self) -> &'static str {
        TrapDef::of(self).name
    }

    /// `"a"` / `"an"` to read correctly before [`TrapEffect::label`].
    pub fn label_article(self) -> &'static str {
        crate::identify::article_for(self.label())
    }
}

// ---------------------------------------------------------------------------
// The trap catalog
// ---------------------------------------------------------------------------

/// One kind of trap, one row: what it is called, how it draws, how often the
/// dungeon lays one, the shallowest floor it lays one on, and — for the two
/// snaring traps — how many turns it holds the victim. The mechanic itself
/// lives in [`spring_trap`], keyed by [`TrapDef::effect`] — a row is
/// description and the numbers its mechanic needs, never behaviour.
///
/// Adding a trap is a row here, a [`TrapEffect`] variant, and an arm in
/// [`spring_trap`]. See `docs/how-to/add-a-trap.md`.
pub struct TrapDef {
    pub effect: TrapEffect,
    pub name: &'static str,
    pub glyph: char,
    pub color: Color,
    /// How often the dungeon lays this one relative to the others it could lay.
    /// Ten is the baseline (see [`crate::spawn::pick_weighted`]).
    pub weight: u32,
    /// The shallowest floor it appears on.
    pub min_depth: u8,
    /// Turns the victim is [`Snare`]d for — bear trap, sleeping gas. `0` for
    /// every trap that does not snare. Kept on the row (not in
    /// `constants.rs`) so a snaring trap is still one file to add.
    pub snare_turns: u32,
}

impl TrapDef {
    /// The row for `effect`. Panics on a variant nobody gave a row.
    pub fn of(effect: TrapEffect) -> &'static TrapDef {
        TRAPS
            .iter()
            .find(|t| t.effect == effect)
            .unwrap_or_else(|| panic!("no trap row for {effect:?}"))
    }

    /// The row called `name`, or `None`.
    pub fn lookup(name: &str) -> Option<&'static TrapDef> {
        TRAPS.iter().find(|t| t.name == name)
    }

    /// A weighted draw from every trap the floor has unlocked.
    pub fn pick(depth: u8, rng: &mut ChaCha12Rng) -> &'static TrapDef {
        let pool: Vec<&TrapDef> = TRAPS
            .iter()
            .filter(|t| t.min_depth <= depth.max(1))
            .collect();
        let weights: Vec<u32> = pool.iter().map(|t| t.weight).collect();
        pool[crate::spawn::pick_weighted(&weights, rng).expect("a depth-1 trap row")]
    }
}

/// The six classic Rogue traps, one row each. Every one is equally likely and
/// available from the first floor; the dials are there so a new trap need not
/// be. `snare_turns` is `0` for everything that does not pin the victim.
#[rustfmt::skip]
pub const TRAPS: &[TrapDef] = &[
    //        effect                       name                 glyph  colour                  wt  dep  snare
    TrapDef { effect: TrapEffect::Trapdoor, name: "trapdoor",          glyph: '^', color: Color::Green,       weight: 10, min_depth: 1, snare_turns: 0 },
    TrapDef { effect: TrapEffect::Bear,     name: "bear trap",         glyph: '^', color: Color::DarkGreen,   weight: 10, min_depth: 1, snare_turns: 3 },
    TrapDef { effect: TrapEffect::Sleep,    name: "sleeping gas trap", glyph: '^', color: Color::Blue,        weight: 10, min_depth: 1, snare_turns: 5 },
    TrapDef { effect: TrapEffect::Teleport, name: "teleport trap",     glyph: '^', color: Color::DarkMagenta, weight: 10, min_depth: 1, snare_turns: 0 },
    TrapDef { effect: TrapEffect::Arrow,    name: "arrow trap",        glyph: '^', color: Color::DarkCyan,    weight: 10, min_depth: 1, snare_turns: 0 },
    TrapDef { effect: TrapEffect::Dart,     name: "dart trap",         glyph: '^', color: Color::Cyan,        weight: 10, min_depth: 1, snare_turns: 0 },
];

impl TrapReveal {
    const ALL: [TrapReveal; 3] = [
        TrapReveal::Sight,
        TrapReveal::Adjacent,
        TrapReveal::Triggered,
    ];
}

/// Which damage tier governs a trap's bite at `depth`: `0` on the shallowest
/// floors, rising at each boundary in [`TRAP_DAMAGE_TIER_LAST_DEPTH`]. Coarser
/// than `map::difficulty_tier` on purpose — a trap steps up three times over a
/// run, not four.
fn trap_damage_tier(depth: u8) -> i32 {
    TRAP_DAMAGE_TIER_LAST_DEPTH
        .iter()
        .position(|&last| depth <= last)
        .unwrap_or(TRAP_DAMAGE_TIER_LAST_DEPTH.len()) as i32
}

/// The player is trying to walk while a sprung bear trap has their leg. Logs
/// the lurch, spends [`BEAR_TRAP_THRASH_DAMAGE`], and leaves the turn consumed.
/// The caller has already ruled out an attack — a swing at an adjacent foe
/// still lands.
///
/// The HP hit is tiny, but the mess is not: the torn leg splatters as if it
/// were a [`BEAR_TRAP_THRASH_GORE`] wound, and an impact spark flashes on the
/// tile.
pub fn bear_trap_thrash(world: &mut World, victim: Entity) {
    world
        .resource_mut::<GameLog>()
        .add("As you try to free yourself, the trap flays your leg.");
    apply_damage(world, victim, BEAR_TRAP_THRASH_DAMAGE);
    spill_blood(world, victim, BEAR_TRAP_THRASH_GORE, false);
    let spot = world.get::<Position>(victim).copied();
    trap_spark(world, spot);
}

/// Whether the player is unable to take *any* action this turn — asleep in gas.
/// A bear trap does **not** count: it blocks movement only (the engine still
/// reads a key, so the player can strike or thrash).
pub fn player_incapacitated(world: &mut World) -> bool {
    matches!(player_snare(world), Some(SnareKind::Sleep))
}

/// Everything a floor trap needs. Deliberately has no [`Item`] — traps are not
/// pickable — and starts [`Hidden`] regardless of reveal style.
#[derive(Bundle)]
pub struct TrapBundle {
    pub name: Name,
    pub glyph: Renderable,
    pub position: Position,
    pub trap: Trap,
    pub hidden: Hidden,
}

impl TrapBundle {
    /// A trap built straight from its catalog row, with the reveal style the
    /// caller wants. The one place a trap entity is described.
    pub fn from_def(def: &TrapDef, reveal: TrapReveal, position: Position) -> Self {
        Self {
            name: Name {
                what: def.name.to_string(),
            },
            glyph: Renderable {
                glyph: def.glyph,
                color: def.color,
            },
            position,
            trap: Trap {
                effect: def.effect,
                reveal,
                revealed: false,
            },
            hidden: Hidden,
        }
    }

    fn new(effect: TrapEffect, reveal: TrapReveal, position: Position) -> Self {
        Self::from_def(TrapDef::of(effect), reveal, position)
    }

    pub fn trapdoor(position: Position) -> Self {
        Self::new(TrapEffect::Trapdoor, TrapReveal::Sight, position)
    }
    pub fn bear(position: Position) -> Self {
        Self::new(TrapEffect::Bear, TrapReveal::Sight, position)
    }
    pub fn sleep(position: Position) -> Self {
        Self::new(TrapEffect::Sleep, TrapReveal::Sight, position)
    }
    pub fn teleport(position: Position) -> Self {
        Self::new(TrapEffect::Teleport, TrapReveal::Sight, position)
    }
    pub fn arrow(position: Position) -> Self {
        Self::new(TrapEffect::Arrow, TrapReveal::Sight, position)
    }
    pub fn dart(position: Position) -> Self {
        Self::new(TrapEffect::Dart, TrapReveal::Sight, position)
    }

    /// The trap a floor at `depth` lays: a weighted draw from [`TRAPS`], with a
    /// uniformly random reveal style.
    pub fn random(rng: &mut ChaCha12Rng, depth: u8, position: Position) -> Self {
        let def = TrapDef::pick(depth, rng);
        let reveal = TrapReveal::ALL[rng.gen_range(0..TrapReveal::ALL.len())];
        Self::from_def(def, reveal, position)
    }
}

/// Ages every [`Snare`] down by one and lifts the ones that reach zero. Runs at
/// the top of the turn so the turn a snare is applied is not also counted.
pub fn snare_system(world: &mut World) {
    if world
        .get_resource::<crate::state::Ending>()
        .is_some_and(|e| e.player_dead)
    {
        return;
    }

    let mut expired: Vec<Entity> = Vec::new();
    let mut ticking: Vec<Entity> = world
        .query_filtered::<Entity, With<Snare>>()
        .iter(world)
        .collect();
    for entity in ticking.drain(..) {
        let Some(mut snare) = world.get_mut::<Snare>(entity) else {
            continue;
        };
        snare.turns = snare.turns.saturating_sub(1);
        if snare.turns == 0 {
            expired.push(entity);
        }
    }

    for entity in expired {
        let kind = world.get::<Snare>(entity).map(|s| s.kind);
        world.entity_mut(entity).remove::<Snare>();
        if world.get::<Player>(entity).is_some() {
            let msg = match kind {
                Some(SnareKind::Bear) => "You wrench your leg free of the bear trap.",
                _ => "You shake off the drowsiness and come to.",
            };
            world.resource_mut::<GameLog>().add(msg);
        }
    }
}

/// Springs any trap whose tile an actor entered this turn, then clears the
/// [`EntityMoved`] markers. Placed just after [`crate::ai`] in the schedule so
/// it sees both the player's move and the monsters'.
pub fn trap_system(world: &mut World) {
    let movers: Vec<Entity> = world
        .query_filtered::<Entity, (With<EntityMoved>, With<Position>)>()
        .iter(world)
        .collect();

    for mover in movers {
        let Some(pos) = world.get::<Position>(mover).copied() else {
            continue;
        };
        let trap = {
            let mut q = world.query_filtered::<(Entity, &Position), With<Trap>>();
            q.iter(world)
                .find(|(_, tp)| tp.x == pos.x && tp.y == pos.y)
                .map(|(e, _)| e)
        };
        if let Some(trap) = trap {
            spring_trap(world, trap, mover);
        }
    }

    let marked: Vec<Entity> = world
        .query_filtered::<Entity, With<EntityMoved>>()
        .iter(world)
        .collect();
    for e in marked {
        world.entity_mut(e).remove::<EntityMoved>();
    }
}

/// Whether the player's viewshed currently covers `(x, y)`.
fn player_sees(world: &mut World, x: u16, y: u16) -> bool {
    world
        .query_filtered::<&Viewshed, With<Player>>()
        .iter(world)
        .next()
        .is_some_and(|v| v.visible_tiles.iter().any(|&(vx, vy)| vx == x && vy == y))
}

/// A short display name for `entity` in a trap log line (`"you"` for the hero).
fn actor_label(world: &World, entity: Entity) -> String {
    if world.get::<Player>(entity).is_some() {
        return "you".to_string();
    }
    world
        .get::<Name>(entity)
        .map(|n| format!("the {}", n.what))
        .unwrap_or_else(|| "something".to_string())
}

/// Fires `trap`'s effect on `victim`, reveals the trap for good, and — for
/// single-shot traps — despawns it.
fn spring_trap(world: &mut World, trap: Entity, victim: Entity) {
    let Some(effect) = world.get::<Trap>(trap).map(|t| t.effect) else {
        return;
    };
    let is_player = world.get::<Player>(victim).is_some();
    let trap_pos = world.get::<Position>(trap).copied();

    // The trap is now known, whether or not the player was the one to find it.
    world.entity_mut(trap).remove::<Hidden>();
    if let Some(mut t) = world.get_mut::<Trap>(trap) {
        t.revealed = true;
    }

    let seen = is_player || trap_pos.is_some_and(|p| player_sees(world, p.x, p.y));

    if seen && !is_player {
        let who = actor_label(world, victim);
        world.resource_mut::<GameLog>().add(format!(
            "{who} steps on {} {}!",
            crate::identify::article_for(effect.label()),
            effect.label()
        ));
    }

    // Snaring traps hold the victim for as many turns as their row says.
    let snare_turns = TrapDef::of(effect).snare_turns;

    match effect {
        TrapEffect::Trapdoor => trapdoor_effect(world, victim, is_player, seen),
        TrapEffect::Bear => {
            snare_victim(world, victim, SnareKind::Bear, snare_turns, is_player);
            // A bear trap only bites once.
            world.entity_mut(trap).despawn();
        }
        TrapEffect::Sleep => snare_victim(world, victim, SnareKind::Sleep, snare_turns, is_player),
        TrapEffect::Teleport => teleport_effect(world, victim, is_player),
        TrapEffect::Arrow => arrow_effect(world, victim, is_player, seen, trap_pos),
        TrapEffect::Dart => dart_effect(world, victim, is_player, seen, trap_pos),
    }
}

/// A one-off impact spark on the trap's tile, if there is an effect layer at
/// all (headless tests run without one).
fn trap_spark(world: &mut World, trap_pos: Option<Position>) {
    if let (Some(p), Some(mut fx)) = (trap_pos, world.get_resource_mut::<Particles>()) {
        fx.hit_spark(p.x, p.y);
    }
}

fn trapdoor_effect(world: &mut World, victim: Entity, is_player: bool, seen: bool) {
    if !is_player {
        if seen {
            let who = actor_label(world, victim);
            world
                .resource_mut::<GameLog>()
                .add(format!("{who} drops through the trapdoor and is gone."));
        }
        world.entity_mut(victim).despawn();
        return;
    }

    let depth = world.resource::<Depth>().what;
    if depth >= FINAL_DEPTH {
        world
            .resource_mut::<GameLog>()
            .add("A trapdoor gapes — but there is only solid rock below. It grinds shut.");
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add("A trapdoor yawns open beneath you!");
    transition_level(world, true, LevelChange::Trapdoor);
}

fn snare_victim(world: &mut World, victim: Entity, kind: SnareKind, turns: u32, is_player: bool) {
    world.entity_mut(victim).insert(Snare { turns, kind });
    if is_player {
        let msg = match kind {
            SnareKind::Bear => {
                "Steel jaws snap shut on your leg — you can't take a step, but your arms are free!"
            }
            SnareKind::Sleep => "Gas billows up around you. Your eyelids turn to lead...",
        };
        world.resource_mut::<GameLog>().add(msg);
    }
}

fn teleport_effect(world: &mut World, victim: Entity, is_player: bool) {
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut pos) = world.get_mut::<Position>(victim) {
            pos.x = x;
            pos.y = y;
        }
        if let Some(mut vs) = world.get_mut::<Viewshed>(victim) {
            vs.dirty = true;
        }
    }
    if is_player {
        world
            .resource_mut::<GameLog>()
            .add("The walls change! You are whisked to a different part of the dungeon.");
    }
}

fn arrow_effect(
    world: &mut World,
    victim: Entity,
    is_player: bool,
    seen: bool,
    trap_pos: Option<Position>,
) {
    let tier = trap_damage_tier(world.resource::<Depth>().what);
    let armor_plus = total_armor_plus(world, victim);
    // 1d4 + 1 at the surface, one more point of head start per depth tier.
    let roll = roll_dice(world, ARROW_DAMAGE_DICE, ARROW_DAMAGE_SIDES)
        + ARROW_DAMAGE_BONUS
        + tier * ARROW_DAMAGE_PER_TIER;
    let damage = (roll - armor_plus).max(0);
    let who = actor_label(world, victim);

    if damage <= 0 {
        if is_player || seen {
            world
                .resource_mut::<GameLog>()
                .add(format!("An arrow whistles past {who} and clatters away."));
        }
        // A missed arrow becomes loot on the trap's tile.
        if let Some(p) = trap_pos {
            world.spawn((
                Name {
                    what: "arrow".to_string(),
                },
                Renderable {
                    glyph: '↑',
                    color: Color::Grey,
                },
                p,
                Item,
                Value { amount: 2 },
            ));
        }
        return;
    }

    if is_player || seen {
        world
            .resource_mut::<GameLog>()
            .add(format!("An arrow plinks into {who} for {damage} damage!"));
    }
    trap_spark(world, trap_pos);
    apply_damage(world, victim, damage);
}

fn dart_effect(
    world: &mut World,
    victim: Entity,
    is_player: bool,
    seen: bool,
    trap_pos: Option<Position>,
) {
    let tier = trap_damage_tier(world.resource::<Depth>().what);
    let armor_plus = total_armor_plus(world, victim);
    let roll = roll_dice(world, DART_DAMAGE_DICE, DART_DAMAGE_SIDES); // 1d2
    let damage = (roll - armor_plus).max(0);
    let who = actor_label(world, victim);

    if damage <= 0 {
        if is_player || seen {
            world
                .resource_mut::<GameLog>()
                .add(format!("A dart glances off {who}."));
        }
        return;
    }

    if is_player || seen {
        world
            .resource_mut::<GameLog>()
            .add(format!("A poisoned dart pricks {who} for {damage} damage!"));
    }
    trap_spark(world, trap_pos);
    apply_damage(world, victim, damage);

    // The poison saps melee power permanently — a hit to the attack die itself,
    // not a modifier — unless something sustains the victim's strength. A potion of restore
    // strength (not yet wired) will heal `power` back up to `max_power`. The
    // deeper the dart, the harder the bite: one point per depth tier.
    if world.get::<SustainsStrength>(victim).is_some() {
        if is_player {
            world
                .resource_mut::<GameLog>()
                .add("The poison burns, but your strength holds firm.");
        }
        return;
    }
    let drain = DART_POWER_DRAIN_BASE + tier * DART_POWER_DRAIN_PER_TIER;
    let drained = world
        .get_mut::<Fighter>(victim)
        .map(|mut f| {
            let before = f.power;
            f.power = (f.power - drain).max(1);
            f.power != before
        })
        .unwrap_or(false);
    if is_player && drained {
        world
            .resource_mut::<GameLog>()
            .add("The poison courses through you — you feel your strength ebb away.");
    }
}

/// A uniformly random walkable tile on the current floor that no actor is
/// standing on. `None` only if the map is somehow wall-to-wall.
pub(crate) fn random_open_tile(world: &mut World) -> Option<(u16, u16)> {
    let occupied: std::collections::HashSet<(u16, u16)> = world
        .query_filtered::<&Position, Or<(With<Player>, With<Mob>)>>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .collect();

    let candidates: Vec<(u16, u16)> = {
        let map = world.resource::<Map>();
        let walkable = |x, y| {
            matches!(
                map.tiles[tile_index(x, y)],
                TileType::Room
                    | TileType::Passage
                    | TileType::Door
                    | TileType::Upstairs
                    | TileType::Downstairs
            )
        };
        (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| walkable(x, y) && !occupied.contains(&(x, y)))
            .collect()
    };

    if candidates.is_empty() {
        return None;
    }
    let idx = world
        .resource_mut::<GameRng>()
        .0
        .gen_range(0..candidates.len());
    Some(candidates[idx])
}

/// Convenience for the engine loop: the kind of snare pinning the player, if
/// any.
pub fn player_snare(world: &mut World) -> Option<SnareKind> {
    world
        .query_filtered::<&Snare, With<Player>>()
        .iter(world)
        .next()
        .map(|s| s.kind)
}
