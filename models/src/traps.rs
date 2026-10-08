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
//! nihilurk has no "wait a turn and search" action, so those three modes are the
//! only ways a trap ever comes to light before it bites.
//!
//! The trap components themselves ([`Trap`], the three holds,
//! [`TrapEffect`], [`TrapReveal`], [`EntityMoved`]) are nouns and live in
//! [`crate::components`]; this module is the verbs — the catalog row
//! ([`TrapDef`]), the spring, and the per-effect mechanics.
//!
//! ## Turn wiring
//!
//! Movement code (the player in `move_player`, monsters in [`crate::ai`]) tags
//! the mover with [`EntityMoved`]. [`trap_system`] runs just after the AI, walks
//! that list, and springs any trap sharing a tile with a mover. [`crate::effects::tick_effects`]
//! runs at the very top of the turn and ages [`Snare`] (bear trap / sleep gas)
//! down, so the turn a snare is applied is never the turn it is decremented.
//!
//! ## Trick shots
//!
//! A trap only *bites* something standing on it. Set one off from across the
//! room — put a missile on its tile, or wash a wand's blast over it — and it
//! has nobody to bite, so the whole mechanism lets go at once instead:
//! [`detonate_trap`] bursts it over [`TRICK_SHOT_RADIUS`] tiles around it, armour-ignoring,
//! and works the trap's own effect on everyone caught. It is nobody's friend;
//! stand a tile away from your own shot and it catches you too.
//!
//! Three rules hold the shot together:
//!
//! * **You can only shoot what you can see.** [`detonate_at`] — the aimed
//!   shot — passes over a trap still [`Hidden`], because lining up a shot on a
//!   mechanism nobody has found is not a trick, it is the dungeon playing
//!   itself. The tell is the other way round: a creature standing on something
//!   a shot could set off is drawn with a magenta cell, so the shot is offered
//!   before it is taken.
//! * **The shot spends what it goes off on.** Trap and coin alike are gone the
//!   instant they let go. The Element of Yoord is the one exception, and it is
//!   the exception to everything.
//! * **Bursts chain.** Anything in a burst that a shot could have set off goes
//!   off with it ([`chain_react`]) — *including* a trap nobody had found, since
//!   the blast does not have to know a mechanism is there to roll over it. Each
//!   link is spent before its own burst opens, so a chain always ends.
//!
//! ## Bear trap
//!
//! A [`crate::effects::Pinned`] snare impedes *movement only*. The victim can still
//! strike an adjacent foe; a step, though, becomes a bloody lurch against the
//! jaws — one wasted turn and [`bear_trap_thrash`]. [`crate::effects::Asleep`] is the
//! total one: no action of any kind.
//!
//! ## Armour rule
//!
//! The damage traps (arrow, dart) deal missile-type damage, so they subtract
//! only the defender's flat bonus — "armour plus", i.e. `armor_bonus` plus any
//! equipped suit's `arm_bonus` (see [`crate::helpers::total_armor_plus`]) — and
//! never the armour die.
//!
//! Their bite also scales with depth, in tiers that end at
//! [`TRAP_DAMAGE_TIER_LAST_DEPTH`]:
//! each tier adds [`ARROW_DAMAGE_PER_TIER`] to the arrow trap's roll and
//! [`DART_POWER_DRAIN_PER_TIER`] to the dart trap's permanent power drain. The
//! dials are [`crate::constants::traps`].

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::*;
use crate::constants::traps::{
    ARROW_DAMAGE_BONUS, ARROW_DAMAGE_DICE, ARROW_DAMAGE_PER_TIER, ARROW_DAMAGE_SIDES,
    BEAR_TRAP_THRASH_DAMAGE, BEAR_TRAP_THRASH_GORE, DART_DAMAGE_DICE, DART_DAMAGE_SIDES,
    DART_POWER_DRAIN_BASE, DART_POWER_DRAIN_PER_TIER, PICKUP_TRICK_SHOT_RADIUS, TRAP_BREAK_CHANCE,
    TRAP_DAMAGE_TIER_LAST_DEPTH, TRICK_SHOT_DAMAGE_DICE, TRICK_SHOT_DAMAGE_SIDES,
    TRICK_SHOT_RADIUS,
};
use crate::effects::{Asleep, Grant, Petrified, Pinned};
use crate::helpers::{
    Hit, apply_damage, apply_hit, leave_smoke, leave_tinted_smoke, player_sees, roll_dice,
    spill_blood, total_armor_plus,
};
use crate::identify::article_for;
use crate::map::{
    Endless, FINAL_DEPTH, GameRng, LevelChange, MAP_HEIGHT, MAP_WIDTH, Map, Smoke, TileType,
    tile_index, transition_level,
};
use crate::particles::{BlastPalette, Particles};
use crate::shake::{ShakeKind, kick_shake};

impl TrapEffect {
    /// The name shown once the trap is known — read straight off the row.
    pub fn label(self) -> &'static str {
        TrapDef::of(self).display_name()
    }

    /// `"a"` / `"an"` to read correctly before [`TrapEffect::label`].
    pub fn label_article(self) -> &'static str {
        article_for(self.label())
    }
}

// ---------------------------------------------------------------------------
// The trap catalog
// ---------------------------------------------------------------------------

/// One kind of trap, one row: what it is called, how it draws, how often the
/// dungeon lays one, the shallowest floor it lays one on, and — for the two
/// snaring traps — how many turns it holds the victim. The mechanic itself
/// lives in [`apply_trap_effect`], keyed by [`TrapDef::effect`] — a row is
/// description and the numbers its mechanic needs, never behaviour.
///
/// Adding a trap is a row here, a [`TrapEffect`] variant, and an arm in each
/// of [`apply_trap_effect`] and [`trap_flourish`]. See
/// `docs/how-to/add-a-trap.md`.
pub struct TrapDef {
    /// The [`TrapEffect`] this row is the entry for.
    pub effect: TrapEffect,
    /// The row's identity, and what the trap is called.
    pub name: &'static str,
    /// The character it draws as once known.
    pub glyph: char,
    /// The colour it draws in.
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

    /// What the player sees this trap called, in whatever language this
    /// binary was built for. `name` itself never changes — see
    /// [`crate::monsters::MonsterDef::display_name`]'s doc comment for why.
    pub fn display_name(&self) -> &'static str {
        strings::content_name(self.name)
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
pub(crate) fn trap_damage_tier(depth: u8) -> i32 {
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
    thrash(world, victim, strings::bear_trap_thrash());
}

/// The same lurch as [`bear_trap_thrash`], for a victim held in a living
/// creature's jaws ([`crate::effects::Clamped`]) rather than a bear trap's —
/// the venus flytrap's bite, and a revealed xeroc's. Same cost, different
/// words: nothing here says "trap".
pub fn clamped_thrash(world: &mut World, victim: Entity) {
    thrash(world, victim, strings::clamped_thrash());
}

fn thrash(world: &mut World, victim: Entity, line: &'static str) {
    world.resource_mut::<GameLog>().add(line);
    apply_damage(world, victim, BEAR_TRAP_THRASH_DAMAGE);
    spill_blood(world, victim, BEAR_TRAP_THRASH_GORE, false);
    let spot = world.get::<Position>(victim).copied();
    trap_spark(world, spot);
}

/// Whether the player is unable to take *any* action this turn — asleep in
/// gas, or standing as stone under a medusa's gaze. A bear trap does **not**
/// count: it blocks movement only (the engine still reads a key, so the player
/// can strike or thrash).
pub fn player_incapacitated(world: &mut World) -> bool {
    player_held_by(world, Grant::of::<Asleep>()) || player_held_by(world, Grant::of::<Petrified>())
}

/// Everything a floor trap needs. Deliberately has no [`Item`] — traps are not
/// pickable — and starts [`Hidden`] regardless of reveal style.
#[derive(Bundle)]
pub struct TrapBundle {
    /// What the trap is called.
    pub name: Name,
    /// How it draws once revealed.
    pub glyph: Renderable,
    /// The tile it lies on.
    pub position: Position,
    /// The trap itself: effect, reveal style and whether it is known.
    pub trap: Trap,
    /// Every trap starts hidden, whatever its reveal style.
    pub hidden: Hidden,
}

impl TrapBundle {
    /// A trap built straight from its catalog row, with the reveal style the
    /// caller wants. The one place a trap entity is described.
    pub fn from_def(def: &TrapDef, reveal: TrapReveal, position: Position) -> Self {
        Self {
            name: Name {
                what: strings::content_name(def.name).to_string(),
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

    /// A trapdoor at `position`, revealed by sight.
    pub fn trapdoor(position: Position) -> Self {
        Self::new(TrapEffect::Trapdoor, TrapReveal::Sight, position)
    }
    /// A bear trap at `position`, revealed by sight.
    pub fn bear(position: Position) -> Self {
        Self::new(TrapEffect::Bear, TrapReveal::Sight, position)
    }
    /// A sleeping-gas trap at `position`, revealed by sight.
    pub fn sleep(position: Position) -> Self {
        Self::new(TrapEffect::Sleep, TrapReveal::Sight, position)
    }
    /// A teleport trap at `position`, revealed by sight.
    pub fn teleport(position: Position) -> Self {
        Self::new(TrapEffect::Teleport, TrapReveal::Sight, position)
    }
    /// An arrow trap at `position`, revealed by sight.
    pub fn arrow(position: Position) -> Self {
        Self::new(TrapEffect::Arrow, TrapReveal::Sight, position)
    }
    /// A poisoned-dart trap at `position`, revealed by sight.
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

/// Springs any trap whose tile an actor entered this turn, then clears the
/// [`EntityMoved`] markers. Placed just after [`crate::ai`] in the schedule so
/// it sees both the player's move and the monsters' — the last reader of
/// [`EntityMoved`] before it clears the tag.
///
/// Assumes every mover this turn already carries [`EntityMoved`]: the player's
/// step tags itself before the schedule runs, and `ai` tags each monster it
/// moves. A mover that skips the tag springs nothing.
pub fn trap_system(world: &mut World) {
    let movers: Vec<Entity> = world
        .query_filtered::<Entity, (With<EntityMoved>, With<Position>)>()
        .iter(world)
        .collect();

    for mover in movers {
        if world.get::<crate::effects::Flies>(mover).is_some() {
            continue;
        }
        let Some(pos) = world.get::<Position>(mover).copied() else {
            continue;
        };
        if let Some(trap) = trap_at(world, pos) {
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

/// A short display name for `entity` in a trap log line (`"you"` for the hero).
fn actor_label(world: &World, entity: Entity) -> String {
    if world.get::<Player>(entity).is_some() {
        return "you".to_string();
    }
    match world.get::<Name>(entity) {
        Some(_) => strings::the(&crate::helpers::item_label(world, entity)),
        None => "something".to_string(),
    }
}

/// The trap sitting on `pos`, if there is one. At most one trap is ever laid
/// on a tile.
pub fn trap_at(world: &mut World, pos: Position) -> Option<Entity> {
    let mut q = world.query_filtered::<(Entity, &Position), With<Trap>>();
    q.iter(world)
        .find(|(_, tp)| tp.x == pos.x && tp.y == pos.y)
        .map(|(e, _)| e)
}

/// Fires `trap`'s effect on `victim`, reveals the trap for good, and despawns
/// it if the mechanism is spent — always for the bear trap, otherwise
/// [`TRAP_BREAK_CHANCE`] of the time.
pub(crate) fn spring_trap(world: &mut World, trap: Entity, victim: Entity) {
    let Some(effect) = world.get::<Trap>(trap).map(|t| t.effect) else {
        return;
    };
    let is_player = world.get::<Player>(victim).is_some();
    let trap_pos = world.get::<Position>(trap).copied();

    world.entity_mut(trap).remove::<Hidden>();
    if let Some(mut t) = world.get_mut::<Trap>(trap) {
        t.revealed = true;
    }

    let seen = is_player || trap_pos.is_some_and(|p| player_sees(world, p.x, p.y));

    if seen && !is_player {
        let who = actor_label(world, victim);
        world.resource_mut::<GameLog>().add(strings::steps_on_trap(
            &who,
            article_for(effect.label()),
            effect.label(),
        ));
    }

    let spent = effect == TrapEffect::Bear
        || world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(TRAP_BREAK_CHANCE);
    if spent {
        world.entity_mut(trap).despawn();
        if seen && effect != TrapEffect::Bear {
            world
                .resource_mut::<GameLog>()
                .add(strings::trap_breaks(effect.label()));
        }
    }

    apply_trap_effect(world, effect, victim, is_player, seen, trap_pos);
}

/// What a trap *does* to one victim, with the trap entity already dealt with by
/// the caller. Split out from [`spring_trap`] because a trap has two ways of
/// going off — something stood on it, or something shot it (see
/// [`detonate_trap`]) — and only the mechanics below are common to both.
///
/// Exhaustive over [`TrapEffect`], deliberately with no catch-all: a trap
/// effect added to the enum and not given an arm here fails the build instead
/// of quietly doing nothing.
fn apply_trap_effect(
    world: &mut World,
    effect: TrapEffect,
    victim: Entity,
    is_player: bool,
    seen: bool,
    trap_pos: Option<Position>,
) {
    let snare_turns = TrapDef::of(effect).snare_turns;

    let on_the_mechanism = trap_pos.is_some() && trap_pos == world.get::<Position>(victim).copied();
    if (is_player || seen) && on_the_mechanism {
        trap_flourish(world, effect, trap_pos);
    }

    match effect {
        TrapEffect::Trapdoor => trapdoor_effect(world, victim, is_player, seen),
        TrapEffect::Bear => snare_victim(
            world,
            victim,
            Grant::of::<Pinned>(),
            snare_turns,
            is_player,
            strings::bear_trap_snare(),
        ),
        TrapEffect::Sleep => snare_victim(
            world,
            victim,
            Grant::of::<Asleep>(),
            snare_turns,
            is_player,
            strings::sleep_gas_snare(),
        ),
        TrapEffect::Teleport => teleport_effect(world, victim, is_player),
        TrapEffect::Arrow => arrow_effect(world, victim, is_player, seen, trap_pos),
        TrapEffect::Dart => dart_effect(world, victim, is_player, seen, trap_pos),
    }
}

// ---------------------------------------------------------------------------
// Trick shots
// ---------------------------------------------------------------------------

/// A trap that something set off *from a distance* — a missile that came down
/// on it, a wand's blast that washed over it — and which therefore has nobody
/// standing on it to bite. So it goes off all at once instead: the whole
/// mechanism lets go in a [`TRICK_SHOT_RADIUS`] burst that deals
/// `TRICK_SHOT_DAMAGE_DICE d TRICK_SHOT_DAMAGE_SIDES` to everything caught —
/// no armour of any kind turns this aside — and then works the trap's own
/// effect on each survivor. Six arrows at once, a lungful of gas for the whole
/// room, a trapdoor that swallows the pack of them.
///
/// The trap is spent either way, and the shot is nobody's friend: stand within
/// a tile of your own trick shot and it catches you too.
///
/// `shooter` is whoever authored the shot, carried through only so the coins
/// this burst chains into know whose it was.
///
/// Returns whether there was a trap here to set off at all, so a caller can
/// hand the same tile to it without checking first.
pub fn detonate_trap(world: &mut World, trap: Entity, shooter: Option<Entity>) -> bool {
    let Some(effect) = world.get::<Trap>(trap).map(|t| t.effect) else {
        return false;
    };
    let Some(center) = world.get::<Position>(trap).copied() else {
        return false;
    };

    world.entity_mut(trap).despawn();
    let victims = burst(
        world,
        center,
        TRICK_SHOT_RADIUS,
        BlastPalette::Force,
        true,
        shooter,
    );

    for victim in victims {
        if world.get::<Fighter>(victim).is_some_and(|f| f.hp <= 0) {
            finish_burst_casualty(world, victim, center, effect);
            continue;
        }
        if world.get::<Position>(victim).is_none() {
            continue;
        }
        let is_player = world.get::<Player>(victim).is_some();
        let seen = player_sees(world, center.x, center.y);
        apply_trap_effect(world, effect, victim, is_player, seen, Some(center));
    }
    true
}

/// The other thing on a floor worth shooting: a coin.
///
/// A pickup has no mechanism to let go, so what goes off is the shot itself —
/// which is why it covers [`PICKUP_TRICK_SHOT_RADIUS`], wider than a trap's
/// [`TRICK_SHOT_RADIUS`].
/// And `shooter` **gets the coin's effect**, across the room, before the burst
/// rolls out: a red coin heals whoever shot it, a gold one pays them, a
/// platinum one makes them its promise. That is the whole appeal — a coin you
/// cannot reach in time is a coin you can still use, and a coin in the middle
/// of a crowd is worth using that way even when you could have walked to it.
///
/// The effect goes nowhere at all when nothing shot it (a coin caught in a
/// chain reaction with no author at the end of it): it is spent, and that is
/// it.
///
/// **The hero coin is the loud one.** Everything it knows comes out at once —
/// the same triple burst the Element of Yoord answers a missile with (see
/// [`ultimate_burst`]), except the coin does not survive saying it. It is the
/// one pickup in the game worth shooting for the shot rather than the payout.
///
/// Returns whether there was a pickup here to set off.
pub fn detonate_pickup(world: &mut World, pickup: Entity, shooter: Option<Entity>) -> bool {
    let Some(center) = world.get::<Position>(pickup).copied() else {
        return false;
    };
    let Some(effect) = world.get::<Pickup>(pickup).map(|p| p.effect) else {
        return false;
    };
    if let Some(shooter) = shooter {
        crate::items::claim_from_afar(world, shooter, pickup);
    }
    world.entity_mut(pickup).despawn();
    if effect == PickupEffect::LearnRandomSpell {
        world
            .resource_mut::<GameLog>()
            .add(strings::hero_coin_ultimate());
        ultimate_burst(world, center, shooter);
        return true;
    }
    burst(
        world,
        center,
        PICKUP_TRICK_SHOT_RADIUS,
        BlastPalette::Force,
        true,
        shooter,
    );
    true
}

/// The ULTIMATE TRICK SHOT: a missile comes down on the Element of Yoord.
///
/// The relic does not break, does not move and is not spent — it is the run,
/// and nothing the player does to it can cost them it. What it does instead is
/// answer. One wide burst where it lies; then a second burst centred on every
/// creature that one caught; then a third on one of them, whoever the reading
/// order reaches first. Each one can catch somebody the last one missed, which
/// is the whole point of firing an arrow at the artifact you came for.
///
/// Returns whether the relic was here to hit.
pub fn ultimate_trick_shot(world: &mut World, relic: Entity, shooter: Option<Entity>) -> bool {
    let Some(center) = world.get::<Position>(relic).copied() else {
        return false;
    };
    world
        .resource_mut::<GameLog>()
        .add(strings::relic_takes_the_hit());
    ultimate_burst(world, center, shooter);
    true
}

/// The three bursts an ULTIMATE TRICK SHOT is made of, with no opinion about
/// what set them off: one wide burst on `center`, a second on every creature
/// that one caught, and a third on whoever the reading order reached first.
///
/// Shared by the two things that can produce one — the relic answering a
/// missile, and a hero coin going out the way it does — because the *shape*
/// of the answer is the same and only the thing that gave it differs.
fn ultimate_burst(world: &mut World, center: Position, shooter: Option<Entity>) {
    let caught = burst(
        world,
        center,
        PICKUP_TRICK_SHOT_RADIUS,
        BlastPalette::Ultimate,
        true,
        shooter,
    );
    let mut echoes = Vec::new();
    for victim in caught {
        let Some(at) = world.get::<Position>(victim).copied() else {
            continue;
        };
        echoes.push(at);
        burst(
            world,
            at,
            TRICK_SHOT_RADIUS,
            BlastPalette::Ultimate,
            false,
            shooter,
        );
    }
    if let Some(&unlucky) = echoes.first() {
        world
            .resource_mut::<GameLog>()
            .add_colored(strings::ultimate_trick_shot_shout(), LogCategory::TrickShot);
        burst(
            world,
            unlucky,
            TRICK_SHOT_RADIUS,
            BlastPalette::Ultimate,
            false,
            shooter,
        );
    }
}

/// One burst of a trick shot: the tiles it covers, the shout, the shake, the
/// animation, and one roll of damage applied whole to everyone standing in it.
/// Returns who was caught, in reading order, so a caller can follow up on them.
///
/// `announce` is for the first burst of a shot only — a shot that goes off
/// three times is still one trick shot and shouts once.
///
/// Ends by setting off everything in its own footprint that a shot could have
/// set off ([`chain_react`]), which is where a chain reaction comes from:
/// every burst in the game is queued here, so there is exactly one place that
/// has to know a blast can find a second mechanism.
fn burst(
    world: &mut World,
    center: Position,
    radius: i32,
    palette: BlastPalette,
    announce: bool,
    shooter: Option<Entity>,
) -> Vec<Entity> {
    burst_of(world, center, radius, palette, announce, shooter, (None, 1))
}

/// [`burst`] with an element on its damage, which an immunity can turn aside,
/// and a multiplier on the one roll: `(element, damage_mult)`.
fn burst_of(
    world: &mut World,
    center: Position,
    radius: i32,
    palette: BlastPalette,
    announce: bool,
    shooter: Option<Entity>,
    (element, damage_mult): (Option<Element>, i32),
) -> Vec<Entity> {
    let cells = burst_cells(world, center, radius);
    let victims = creatures_in(world, &cells);
    let caught_player = victims.iter().any(|&v| world.get::<Player>(v).is_some());
    let seen = caught_player || player_sees(world, center.x, center.y);

    if seen && announce {
        let shout = if caught_player {
            strings::trick_shot_shout_self()
        } else {
            strings::trick_shot_shout_other()
        };
        world
            .resource_mut::<GameLog>()
            .add_colored(strings::trick_shot_line(shout), LogCategory::TrickShot);
        kick_shake(world, ShakeKind::Hit);
    }
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        let span = fx.explosion(&cells, palette);
        if palette == BlastPalette::Ultimate {
            fx.smoke_burst(&cells);
        }
        fx.hold(span);
    }
    if palette == BlastPalette::Ultimate {
        let mut smoke = world.resource_mut::<Smoke>();
        for &(x, y, _) in &cells {
            smoke.puff(x, y, crate::constants::wands::SMOKE_LINGER_TURNS);
        }
    }

    let damage = roll_dice(world, TRICK_SHOT_DAMAGE_DICE, TRICK_SHOT_DAMAGE_SIDES) * damage_mult;
    for &victim in &victims {
        match element {
            Some(el) => {
                apply_hit(world, victim, Hit::elemental(damage, el), None);
            }
            None => apply_damage(world, victim, damage),
        }
    }
    let area = cells.iter().map(|&(x, y, _)| (x, y)).collect();
    chain_react(world, &area, shooter, false);
    victims
}

/// Everything under a burst that a shot could have set off, set off: the chain
/// reaction.
///
/// A trap nobody has found is as good a link as one they have. The rule about
/// [`Hidden`] traps is about *aiming* at one ([`detonate_at`]) — a blast
/// rolling over a tile does not have to know what is buried in it.
///
/// This terminates because every link is spent — despawned — before its own
/// burst goes off, so nothing is ever a link twice and a floor holds finitely
/// many of them. The Element of Yoord is deliberately not in the chain: it is
/// never spent, and a burst that reached it would answer itself forever.
///
/// Anything that blows up, flies or zaps ends here with the tiles it covered,
/// whatever their number: a blast's disc, a bolt's line, a dart's one tile.
/// `fire` says the thing was a fire blast, which scorches the ice cubes in its
/// way ([`scorch_ice_cube`]).
pub(crate) fn chain_react(
    world: &mut World,
    area: &std::collections::HashSet<(u16, u16)>,
    shooter: Option<Entity>,
    fire: bool,
) {
    let cubes = things_in::<crate::ice::IceCube>(world, area);
    chain_react_with(world, area, shooter, fire, cubes);
}

/// [`chain_react`] with the cubes chosen by the caller. A cold blast that
/// kills something freezes it inside its own footprint, and that cube is
/// the blast's work, not a link in it: the caller names the cubes that stood
/// there *before* it hurt anyone.
pub(crate) fn chain_react_with(
    world: &mut World,
    area: &std::collections::HashSet<(u16, u16)>,
    shooter: Option<Entity>,
    fire: bool,
    cubes: Vec<Entity>,
) {
    for trap in things_in::<Trap>(world, area) {
        detonate_trap(world, trap, shooter);
    }
    for coin in things_in::<Pickup>(world, area) {
        detonate_pickup(world, coin, shooter);
    }
    for potion in things_in::<Potion>(world, area) {
        crate::items::detonate_potion(world, potion, shooter);
    }
    for cube in cubes {
        match fire {
            true => scorch_ice_cube(world, cube, shooter),
            false => detonate_ice_cube(world, cube, shooter),
        };
    }
}

/// An ice cube something shot: it shatters ([`crate::ice`]) and the cold it was
/// holding goes off as a standard trick shot, `TRICK_SHOT_DAMAGE_DICE d
/// TRICK_SHOT_DAMAGE_SIDES` of [`Element::Cold`] over [`TRICK_SHOT_RADIUS`].
/// What the cold kills freezes in its turn.
///
/// Returns whether there was a cube here to set off.
pub fn detonate_ice_cube(world: &mut World, cube: Entity, shooter: Option<Entity>) -> bool {
    set_off_ice_cube(world, cube, shooter, false)
}

/// [`detonate_ice_cube`] for a cube that fire reached: the sudden thaw doubles
/// the radius and the damage, and the burst is still cold. That is
/// intentional.
pub fn scorch_ice_cube(world: &mut World, cube: Entity, shooter: Option<Entity>) -> bool {
    set_off_ice_cube(world, cube, shooter, true)
}

fn set_off_ice_cube(
    world: &mut World,
    cube: Entity,
    shooter: Option<Entity>,
    scorched: bool,
) -> bool {
    if world.get::<crate::ice::IceCube>(cube).is_none() {
        return false;
    }
    let Some(center) = world.get::<Position>(cube).copied() else {
        return false;
    };
    world.entity_mut(cube).despawn();
    crate::ice::shatter(world, center);
    let (radius, mult, palette) = match scorched {
        true => (TRICK_SHOT_RADIUS * 2, 2, BlastPalette::Fire),
        false => (TRICK_SHOT_RADIUS, 1, BlastPalette::Frost),
    };
    let victims = burst_of(
        world,
        center,
        radius,
        palette,
        true,
        shooter,
        (Some(Element::Cold), mult),
    );
    for victim in victims {
        if world.get::<Fighter>(victim).is_some_and(|f| f.hp <= 0) {
            let player_was_alive = world.get::<Player>(victim).is_some()
                && !world.resource::<crate::state::Ending>().player_dead;
            crate::combat::finish_indirect_kill(world, victim, Some(center));
            if player_was_alive {
                world.resource_mut::<crate::state::Ending>().cause =
                    strings::killed_by_ice_cube().to_string();
            }
        }
    }
    true
}

/// Everything carrying `C` standing on one of `cells`. Collected up front
/// because setting one off mutates the world out from under the query.
pub(crate) fn things_in<C: Component>(
    world: &mut World,
    cells: &std::collections::HashSet<(u16, u16)>,
) -> Vec<Entity> {
    world
        .query_filtered::<(Entity, &Position), With<C>>()
        .iter(world)
        .filter(|(_, p)| cells.contains(&(p.x, p.y)))
        .map(|(e, _)| e)
        .collect()
}

/// Whatever is lying on `pos` that a shot can set off, set off. This is the
/// whole of "what happens where the missile lands", so a thrown dagger, a
/// loosed arrow and a wand's blast all get the same answers.
///
/// `shooter` is whoever loosed it, and matters for exactly one of the three
/// answers: a coin pays its effect to them.
///
/// A trap still [`Hidden`] is passed straight over. This is the *aimed* shot,
/// and you cannot aim at a mechanism nobody has found — the shot would be the
/// dungeon setting off its own trap on the player's behalf. Blasts and chain
/// reactions are under no such rule ([`chain_react`]).
///
/// Returns what went off, or `None` for a tile with nothing on it worth
/// hitting.
pub fn detonate_at(world: &mut World, pos: Position, shooter: Option<Entity>) -> Option<TrickShot> {
    if let Some(trap) = trap_at(world, pos).filter(|&t| world.get::<Hidden>(t).is_none()) {
        detonate_trap(world, trap, shooter);
        return Some(TrickShot::Trap);
    }
    if let Some(pickup) = thing_at::<Pickup>(world, pos) {
        detonate_pickup(world, pickup, shooter);
        return Some(TrickShot::Pickup);
    }
    if let Some(potion) = thing_at::<Potion>(world, pos) {
        crate::items::detonate_potion(world, potion, shooter);
        return Some(TrickShot::Potion);
    }
    if let Some(cube) = thing_at::<crate::ice::IceCube>(world, pos) {
        detonate_ice_cube(world, cube, shooter);
        return Some(TrickShot::IceCube);
    }
    if let Some(relic) = thing_at::<Amulet>(world, pos) {
        ultimate_trick_shot(world, relic, shooter);
        return Some(TrickShot::Ultimate);
    }
    None
}

/// What a shot set off. The thrower's own comment on it is
/// `crate::items::throw_system`'s business — this only says what happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrickShot {
    /// A trap the player already knew about went off.
    Trap,
    /// A coin or other pickup went off, paying its effect to the shooter.
    Pickup,
    /// A potion on the floor shattered over the area around it.
    Potion,
    /// An ice cube shattered into a burst of cold.
    IceCube,
    /// The Element of Yoord answered with its burst. It survives.
    Ultimate,
}

/// The first entity carrying `C` standing on `pos`.
fn thing_at<C: Component>(world: &mut World, pos: Position) -> Option<Entity> {
    let mut q = world.query_filtered::<(Entity, &Position), With<C>>();
    q.iter(world)
        .find(|(_, p)| p.x == pos.x && p.y == pos.y)
        .map(|(e, _)| e)
}

/// Finishes one creature the burst killed, here rather than at the reaper's
/// next sweep, so the corpse is flung away from the trap it was standing next
/// to — and so the trap's effect is never worked on something already dead.
/// A dead player gets the trap named on their tombstone; "killer unknown" is a
/// poor epitaph for a shot you lined up yourself.
fn finish_burst_casualty(world: &mut World, victim: Entity, center: Position, effect: TrapEffect) {
    let is_player = world.get::<Player>(victim).is_some();
    let already_dead = world
        .get_resource::<crate::state::Ending>()
        .is_some_and(|e| e.player_dead);
    crate::combat::finish_indirect_kill(world, victim, Some(center));
    if !is_player || already_dead {
        return;
    }
    if let Some(mut ending) = world.get_resource_mut::<crate::state::Ending>() {
        ending.cause = strings::blown_up_by(effect.label_article(), effect.label());
    }
}

/// Every tile a trick shot's burst covers: the trap's own and each open tile
/// within [`TRICK_SHOT_RADIUS`] of it, tagged with its distance from the centre
/// so the animation ripples outward. Walls are not covered — the blast rolls
/// into the room, not through the stone.
fn burst_cells(world: &World, center: Position, radius: i32) -> Vec<(u16, u16, f32)> {
    let map = world.resource::<Map>();
    let mut cells = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let Some((x, y)) = crate::particles::on_map(center.x as i32 + dx, center.y as i32 + dy)
            else {
                continue;
            };
            if map.blocks(x, y) {
                continue;
            }
            cells.push((x, y, ((dx * dx + dy * dy) as f32).sqrt()));
        }
    }
    cells
}

/// Every creature standing in `cells`, in reading order — top row first, then
/// left to right. Fixed on purpose: the arms that roll dice roll them once per
/// victim, so the order they are worked in has to be the same on a replay as it
/// was on the run.
fn creatures_in(world: &mut World, cells: &[(u16, u16, f32)]) -> Vec<Entity> {
    let area: std::collections::HashSet<(u16, u16)> =
        cells.iter().map(|&(x, y, _)| (x, y)).collect();
    let mut caught: Vec<(u16, u16, Entity)> = world
        .query_filtered::<(Entity, &Position), Or<(With<Player>, With<Mob>)>>()
        .iter(world)
        .filter(|(_, p)| area.contains(&(p.x, p.y)))
        .map(|(e, p)| (p.y, p.x, e))
        .collect();
    caught.sort_unstable();
    caught.into_iter().map(|(_, _, e)| e).collect()
}

/// A one-off impact spark on the trap's tile, if there is an effect layer at
/// all (headless tests run without one).
fn trap_spark(world: &mut World, trap_pos: Option<Position>) {
    if let (Some(p), Some(mut fx)) = (trap_pos, world.get_resource_mut::<Particles>()) {
        fx.hit_spark(p.x, p.y);
    }
}

pub(crate) fn trapdoor_effect(world: &mut World, victim: Entity, is_player: bool, seen: bool) {
    if !is_player {
        if seen {
            let who = actor_label(world, victim);
            world
                .resource_mut::<GameLog>()
                .add(strings::drops_through_trapdoor(&who));
        }
        if let Some(pos) = world.get::<Position>(victim).copied() {
            leave_smoke(world, pos);
        }
        world.entity_mut(victim).despawn();
        return;
    }

    let depth = world.resource::<Depth>().what;
    if depth >= FINAL_DEPTH && !world.resource::<Endless>().enabled {
        world
            .resource_mut::<GameLog>()
            .add(strings::trapdoor_grinds_shut());
        let here = world.get::<Position>(victim).copied();
        dust_puff(world, here);
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::trapdoor_yawns_open());
    transition_level(world, true, LevelChange::Trapdoor);
    let landed = world.get::<Position>(victim).copied();
    dust_puff(world, landed);
}

/// Dust on a tile, if there is an effect layer at all (headless tests run
/// without one).
fn dust_puff(world: &mut World, pos: Option<Position>) {
    if let (Some(p), Some(mut fx)) = (pos, world.get_resource_mut::<Particles>()) {
        fx.poof(p.x, p.y, 0.0);
    }
}

/// Holds `victim` and, when it is the player, says how it felt. The sentence
/// belongs to the trap that sprang rather than to the hold itself — the same
/// jaws read differently from a scroll's words — so it is passed in.
fn snare_victim(
    world: &mut World,
    victim: Entity,
    grant: Grant,
    turns: u32,
    is_player: bool,
    line: &str,
) {
    crate::conditions::snare(world, victim, grant, turns);
    if is_player {
        world.resource_mut::<GameLog>().add(line.to_string());
    }
}

fn teleport_effect(world: &mut World, victim: Entity, is_player: bool) {
    let was = world.get::<Position>(victim).copied();
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut pos) = world.get_mut::<Position>(victim) {
            pos.x = x;
            pos.y = y;
        }
        if let Some(mut vs) = world.get_mut::<Viewshed>(victim) {
            vs.dirty = true;
        }
    }
    if let Some(was) = was {
        leave_tinted_smoke(world, was, Color::Magenta);
    }
    if is_player {
        world
            .resource_mut::<GameLog>()
            .add(strings::teleport_trap_whisked());
    }
}

/// What a trap looks like going off, before its mechanic works on anybody.
///
/// Exhaustive over [`TrapEffect`] with no catch-all, the same way
/// [`apply_trap_effect`] is: a trap added to the enum has to answer "and what
/// does it look like?" or fail the build. Two of the six answer it further
/// down instead, where they know where the victim ended up, and say so here
/// rather than going unlisted.
fn trap_flourish(world: &mut World, effect: TrapEffect, trap_pos: Option<Position>) {
    let Some(p) = trap_pos else {
        return;
    };
    match effect {
        TrapEffect::Arrow | TrapEffect::Dart => {
            kick_shake(world, ShakeKind::Hit);
            missile_flourish(world, p, TrapDef::of(effect).color)
        }
        TrapEffect::Bear => {
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.spark_burst(p.x, p.y, Color::DarkGreen);
                fx.condition_mark(p.x, p.y, '#', Color::DarkGreen, BEAR_MARK_DELAY_MS);
            }
        }
        TrapEffect::Sleep => {
            let cells = burst_cells(world, p, 1);
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.smoke_burst(&cells);
                fx.condition_mark(p.x, p.y, 'z', Color::Blue, SLEEP_MARK_DELAY_MS);
            }
        }
        TrapEffect::Teleport | TrapEffect::Trapdoor => {}
    }
}

/// How long after the jaws snap the held glyph shows, and how long after the
/// gas billows the sleep mark does. Both read as a consequence of the beat
/// before rather than part of it.
const BEAR_MARK_DELAY_MS: f32 = 60.0;
const SLEEP_MARK_DELAY_MS: f32 = 120.0;

/// The one embellishment a shooting trap's classic thwack was missing: the
/// bolt visibly arriving from off in the dark rather than simply appearing at
/// the impact tile. Picks one of the four cardinal directions and traces a
/// short flight in toward the trap's own tile — purely cosmetic, the damage is
/// decided elsewhere, so a headless world (no [`Particles`] resource) just
/// skips it.
fn missile_flourish(world: &mut World, p: Position, color: Color) {
    const REACH: i32 = 6;
    const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let (dx, dy) = {
        let mut rng = world.resource_mut::<GameRng>();
        DIRS[rng.0.gen_range(0..DIRS.len())]
    };
    let mut cells = Vec::new();
    for step in (1..=REACH).rev() {
        if let Some(cell) = crate::particles::on_map(p.x as i32 - dx * step, p.y as i32 - dy * step)
        {
            cells.push(cell);
        }
    }
    cells.push((p.x, p.y));
    let Some(mut fx) = world.get_resource_mut::<Particles>() else {
        return;
    };
    fx.hurl(&cells, '↑', color);
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
    let roll = roll_dice(world, ARROW_DAMAGE_DICE, ARROW_DAMAGE_SIDES)
        + ARROW_DAMAGE_BONUS
        + tier * ARROW_DAMAGE_PER_TIER;
    let damage = (roll - armor_plus).max(0);
    let who = actor_label(world, victim);

    if damage <= 0 {
        if is_player || seen {
            world
                .resource_mut::<GameLog>()
                .add(strings::arrow_whistles_past(&who));
        }
        if let Some(p) = trap_pos {
            world.spawn((
                Name {
                    what: strings::content_name("arrow").to_string(),
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
            .add(strings::arrow_plinks(&who, damage));
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
    let roll = roll_dice(world, DART_DAMAGE_DICE, DART_DAMAGE_SIDES);
    let damage = (roll - armor_plus).max(0);
    let who = actor_label(world, victim);

    if damage <= 0 {
        if is_player || seen {
            world
                .resource_mut::<GameLog>()
                .add(strings::dart_glances_off(&who));
        }
        return;
    }

    if is_player || seen {
        world
            .resource_mut::<GameLog>()
            .add(strings::dart_pricks(&who, damage));
    }
    trap_spark(world, trap_pos);
    apply_damage(world, victim, damage);

    let drain = DART_POWER_DRAIN_BASE + tier * DART_POWER_DRAIN_PER_TIER;
    let line = match crate::conditions::drain_power(world, victim, drain, Some(1)) {
        crate::conditions::Drain::Resisted => strings::dart_poison_resisted(),
        crate::conditions::Drain::Took => strings::dart_poison_took(),
        crate::conditions::Drain::Nothing => return,
    };
    if is_player {
        world.resource_mut::<GameLog>().add(line);
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

/// Whether the player is currently held by `grant` — asleep, pinned or
/// rooted. The engine loop asks all three to decide what a movement key can
/// still do.
pub fn player_held_by(world: &mut World, grant: Grant) -> bool {
    let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    else {
        return false;
    };
    grant.probe(world, player)
}
