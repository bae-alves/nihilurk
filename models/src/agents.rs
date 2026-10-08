//! How every mob makes up its mind: a percept in, one action out, and nothing
//! remembered in between.
//!
//! Each turn [`crate::ai`](mod@crate::ai) builds a [`Percept`] for a mob — what is true of it
//! and around it *right now* — and hands it to [`think`] along with the mob's
//! [`RuleSet`]. A rule set is an ordered list of [`Rule`]s. The first rule that
//! fires decides the turn; if none does, the mob waits. No rule reads anything
//! but the percept, and no mob carries anything from one turn to the next that
//! a rule could read: this is a reflex agent, not a planner.
//!
//! **A mob perceives only while it stands in the player's view.** Out of view it
//! does nothing, with two exceptions that do not need a percept to act on: an
//! [`Aggravated`](crate::components::Aggravated) monster makes a beeline for
//! the noise, and the player's [`Helper`](crate::components::Helper) heels. Both go back to their rule set the moment they are in view.
//!
//! [`ai`](mod@crate::ai) keeps the clock (energy, rounds), the gates (asleep,
//! stone) and the hands ([`Action`] into the world). This module never touches
//! the world: every rule is a plain function of the percept, which is what
//! lets each set be tested on a percept built by hand.
//!
//! The sets, what picks them, and every rule are listed in
//! `docs/reference/agents.md`.

use bevy_ecs::entity::Entity;

use crate::catalog::SpellDef;
use crate::components::{MovementType, Position, SpellEffect, SpellKind};
use crate::constants::monsters::MONSTER_SHOT_RANGE;
use crate::constants::wands::BLAST_RADIUS;
use crate::helpers::{chebyshev, get_line};
use crate::map::Map;

/// Something a mob can see that it would fight: the player, or anyone on the
/// other side.
#[derive(Clone, Copy, Debug)]
pub struct Sighting {
    /// The one seen.
    pub who: Entity,
    /// Where they stand.
    pub at: Position,
    /// Whether it is the player rather than another creature.
    pub is_player: bool,
}

/// Everything a mob knows this turn. Built fresh by [`crate::ai`](mod@crate::ai) every time
/// the mob gets to act, and thrown away after.
pub struct Percept<'a> {
    /// Where the mob stands.
    pub at: Position,
    /// Whether the mob stands on a tile the player can see. When it does not,
    /// [`think`] never consults the rule set.
    pub in_view: bool,
    /// Where the player stands, noticed or not.
    pub player_at: Position,
    /// Whether it has noticed the player: in view, and close enough that a
    /// ring of stealth no longer hides them.
    pub noticed: bool,
    /// Held where it stands: it can strike, but it cannot step or shoot.
    pub pinned: bool,
    /// Deep water is floor to it.
    pub swims: bool,
    /// Terrain means nothing to it: it walks through walls and water.
    pub phasing: bool,
    /// It holds a launcher, drawn.
    pub launcher: bool,
    /// The spells it can cast: those in its
    /// [`Spellset`](crate::components::Spellset) its [`Magic`](crate::components::Magic)
    /// can pay for.
    pub spellset: Vec<SpellEffect>,
    /// A die already rolled for this turn, for the rules that choose at random
    /// (which spell to try, which way to stagger). Rolled with the percept so a
    /// rule stays a plain function of what it is handed.
    pub roll: u32,
    /// The player's Helper. Out of view, it heels.
    pub helper: bool,
    /// On the player's side ([`Faction::Ally`](crate::components::Faction)):
    /// its spells never go where they would catch the player.
    pub ally: bool,
    /// Where an [`Aggravated`](crate::components::Aggravated) mob is heading.
    /// Out of view, it goes there.
    pub aggravated: Option<Position>,
    /// Everything it would fight that stands where the player can see, nearest
    /// first, ties broken by tile. The player is only in here once noticed.
    pub foes: Vec<Sighting>,
    /// The floor, for asking what can be walked on or seen through.
    pub map: &'a Map,
}

/// What a mob does with its turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Swing at someone next to it.
    Strike(Entity),
    /// Loose the launcher in its hand at someone.
    Shoot(Entity),
    /// Cast a spell it was born with at a tile.
    Cast(SpellEffect, Position),
    /// Take one step. Into someone it would fight, that is a swing.
    Step(i16, i16),
    /// Do nothing this turn.
    Wait,
}

/// One reflex: a condition on the percept and the action it calls for, in one
/// function that answers `None` when the condition does not hold.
pub struct Rule {
    /// A label for the reflex. Nothing matches on it.
    pub name: &'static str,
    /// The condition and its action: `Some` when the reflex fires.
    pub fire: fn(&Percept) -> Option<Action>,
}

/// A mob's whole mind: rules in the order they are tried.
pub struct RuleSet {
    /// A label for the mind. Nothing matches on it.
    pub name: &'static str,
    /// Whether a step out of a room into a doorway or a corridor is refused —
    /// the room leash that keeps a chaser from following the player out.
    pub leashed: bool,
    /// The reflexes in the order they are tried. The first to fire wins.
    pub rules: &'static [Rule],
}

/// The turn, decided: out of view, the off-view rule; in view, the first rule
/// in `set` that fires, or [`Action::Wait`].
pub fn think(p: &Percept, set: &RuleSet) -> Action {
    if !p.in_view {
        return off_view(p);
    }
    set.rules
        .iter()
        .find_map(|rule| (rule.fire)(p))
        .unwrap_or(Action::Wait)
}

/// Whether a step this turn is held by the room leash: only in view, and only
/// for a set that is leashed.
pub fn leashed(p: &Percept, set: &RuleSet) -> bool {
    p.in_view && set.leashed
}

/// Out of the player's view: an aggravated mob beelines for the noise, a Helper
/// heels, anything else waits.
fn off_view(p: &Percept) -> Action {
    if let Some(goal) = p.aggravated {
        let step = toward(p.at, goal);
        return match step {
            (0, 0) => Action::Wait,
            (dx, dy) => Action::Step(dx, dy),
        };
    }
    if p.helper {
        return heel(p).unwrap_or(Action::Wait);
    }
    Action::Wait
}

/// The rule set a mob thinks with: a Helper's own, otherwise the one its
/// tactic names.
pub fn rule_set_for(movement: MovementType, helper: bool) -> &'static RuleSet {
    if helper {
        return &HELPER;
    }
    match movement {
        MovementType::Static => &STILL,
        MovementType::Chase | MovementType::Aggravated { .. } => &CHASER,
        MovementType::Flee => &FLEER,
        MovementType::Confused => &STAGGERER,
        MovementType::Ambush => &AMBUSHER,
    }
}

// ---------------------------------------------------------------------------
// The sets
// ---------------------------------------------------------------------------

/// Never acts. The test dummy.
pub static STILL: RuleSet = RuleSet {
    name: "still",
    leashed: false,
    rules: &[],
};

/// The basic hunter. Picks a spell from its spellset at random and fires it if
/// it can; otherwise shoots if it can, and otherwise goes to melee — strikes
/// what is next to it, or hunts the player (only a player it has noticed, and
/// never out of its room). It does not care who else its spell catches.
pub static CHASER: RuleSet = RuleSet {
    name: "chaser",
    leashed: true,
    rules: &[CAST, SHOOT, STRIKE, HUNT],
};

/// Lies in wait and strikes only what comes alongside.
pub static AMBUSHER: RuleSet = RuleSet {
    name: "ambusher",
    leashed: false,
    rules: &[STRIKE],
};

/// Walks away from a player it has noticed. Scared for good.
pub static FLEER: RuleSet = RuleSet {
    name: "fleer",
    leashed: false,
    rules: &[FLEE],
};

/// Staggers about at random, and swings at whatever it staggers into.
pub static STAGGERER: RuleSet = RuleSet {
    name: "staggerer",
    leashed: false,
    rules: &[STAGGER],
};

/// The player's Helper: the chaser's loop pointed at the monsters — a spell
/// from its spellset if it can fire one without catching the player, a shot if
/// it can, melee with the nearest monster in view — and with nothing to fight,
/// back to the player's side.
pub static HELPER: RuleSet = RuleSet {
    name: "helper",
    leashed: false,
    rules: &[CAST, SHOOT, STRIKE, CLOSE_IN, HEEL],
};

// ---------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------

/// A launcher drawn and a foe in range with a clear line: shoot the nearest
/// such foe. Not while held — drawing a bow takes feet.
pub const SHOOT: Rule = Rule {
    name: "shoot",
    fire: shoot,
};

/// One spell picked at random from the spellset, fired at the nearest foe it
/// can reach: in its range, with a clear line. An ally also passes over any
/// foe whose blast would catch the player — every time, whatever the spell; a
/// monster does not care who else it catches. Only attack spells with a range
/// are fired this way.
pub const CAST: Rule = Rule {
    name: "cast",
    fire: cast,
};

/// A foe right alongside: strike it, the player before anyone else.
pub const STRIKE: Rule = Rule {
    name: "strike",
    fire: strike,
};

/// A noticed player: take the first step of the shortest walk to them.
pub const HUNT: Rule = Rule {
    name: "hunt",
    fire: hunt,
};

/// Any foe in view: take the first step of the shortest walk to the nearest.
pub const CLOSE_IN: Rule = Rule {
    name: "close in",
    fire: close_in,
};

/// More than a step from the player: take the first step of the shortest walk
/// back to their side.
pub const HEEL: Rule = Rule {
    name: "heel",
    fire: heel,
};

/// A noticed player: step straight away from them.
pub const FLEE: Rule = Rule {
    name: "flee",
    fire: flee,
};

/// Always: one step north, south, east or west, at random.
pub const STAGGER: Rule = Rule {
    name: "stagger",
    fire: stagger,
};

fn shoot(p: &Percept) -> Option<Action> {
    if !p.launcher || p.pinned {
        return None;
    }
    p.foes
        .iter()
        .find(|f| chebyshev(p.at, f.at) <= MONSTER_SHOT_RANGE && clear_line(p.map, p.at, f.at))
        .map(|f| Action::Shoot(f.who))
}

fn cast(p: &Percept) -> Option<Action> {
    if p.spellset.is_empty() {
        return None;
    }
    let spell = p.spellset[p.roll as usize % p.spellset.len()];
    let def = SpellDef::of(spell);
    if !matches!(def.kind, SpellKind::Attack) || def.range == 0 {
        return None;
    }
    p.foes
        .iter()
        .find(|f| {
            chebyshev(p.at, f.at) <= def.range
                && clear_line(p.map, p.at, f.at)
                && !(p.ally && reaches(p.map, spell, f.at, p.player_at))
        })
        .map(|f| Action::Cast(spell, f.at))
}

fn strike(p: &Percept) -> Option<Action> {
    let alongside = |f: &&Sighting| chebyshev(p.at, f.at) == 1;
    p.foes
        .iter()
        .filter(alongside)
        .find(|f| f.is_player)
        .or_else(|| p.foes.iter().find(alongside))
        .map(|f| Action::Strike(f.who))
}

fn hunt(p: &Percept) -> Option<Action> {
    let player = p.foes.iter().find(|f| f.is_player)?;
    route(p, |x, y| (x, y) == (player.at.x, player.at.y), player.at)
}

fn close_in(p: &Percept) -> Option<Action> {
    let foe = p.foes.first()?;
    route(p, |x, y| (x, y) == (foe.at.x, foe.at.y), foe.at)
}

fn heel(p: &Percept) -> Option<Action> {
    if chebyshev(p.at, p.player_at) <= 1 {
        return None;
    }
    route(
        p,
        |x, y| chebyshev(Position { x, y }, p.player_at) == 1,
        p.player_at,
    )
}

fn flee(p: &Percept) -> Option<Action> {
    let player = p.foes.iter().find(|f| f.is_player)?;
    let (dx, dy) = toward(p.at, player.at);
    Some(Action::Step(-dx, -dy))
}

fn stagger(p: &Percept) -> Option<Action> {
    const DIRS: [(i16, i16); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let (dx, dy) = DIRS[p.roll as usize % DIRS.len()];
    Some(Action::Step(dx, dy))
}

// ---------------------------------------------------------------------------
// What the rules share
// ---------------------------------------------------------------------------

/// The one-tile step from `from` straight at `to`, each axis clamped to one.
fn toward(from: Position, to: Position) -> (i16, i16) {
    (
        (to.x as i16 - from.x as i16).signum(),
        (to.y as i16 - from.y as i16).signum(),
    )
}

/// The first step of the shortest walk over ground the mob can stand on to
/// any tile satisfying `goal` (any tile at all, for a phasing mob), biased toward `bias` when several routes tie.
/// A mob knows the dungeon it stands in, so unlike the player it is not
/// limited to tiles it has seen. Reuses auto-explore's own search.
fn route(p: &Percept, goal: impl Fn(u16, u16) -> bool, bias: Position) -> Option<Action> {
    crate::autoexplore::first_step(
        p.at.x,
        p.at.y,
        |x, y| p.phasing || p.map.walkable(x, y, p.swims),
        |fx, fy, tx, ty| p.phasing || p.map.diagonal_step_ok(fx, fy, tx, ty),
        goal,
        Some((bias.x, bias.y)),
    )
    .map(|(dx, dy)| Action::Step(dx, dy))
}

/// Whether a shot could travel clean from `from` to `to` — no wall standing in
/// the way. Bodies in the line do not count: a monster's own kin are not a good
/// enough reason to hold its fire.
pub fn clear_line(map: &Map, from: Position, to: Position) -> bool {
    get_line(from, to)
        .into_iter()
        .filter(|&t| t != from && t != to)
        .all(|t| !map.blocks(t.x, t.y))
}

/// Whether `spell`, cast at `target`, would reach `who`. A spell whose
/// footprint this does not know is assumed to reach everyone, so nothing casts
/// it blind.
fn reaches(map: &Map, spell: SpellEffect, target: Position, who: Position) -> bool {
    match spell {
        SpellEffect::DragonBreath => crate::items::blast_cells(map, target, BLAST_RADIUS)
            .iter()
            .any(|&(x, y, _)| (x, y) == (who.x, who.y)),
        SpellEffect::Thunderbolt => target == who,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MAP_TILE_COUNT, TileType, tile_index};
    use bevy_ecs::world::World;
    use fixedbitset::FixedBitSet;

    /// Walls everywhere, with one open room at x 5..=15, y 4..=8.
    fn room() -> Map {
        let mut map = Map {
            tiles: vec![TileType::Wall; MAP_TILE_COUNT],
            dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            inert_doors: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            special: vec![None; MAP_TILE_COUNT],
            level: None,
        };
        for y in 4..=8 {
            for x in 5..=15 {
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
        map
    }

    fn entities(n: usize) -> Vec<Entity> {
        let mut w = World::new();
        (0..n).map(|_| w.spawn_empty().id()).collect()
    }

    fn percept(map: &Map, at: (u16, u16), player_at: (u16, u16)) -> Percept<'_> {
        Percept {
            at: Position { x: at.0, y: at.1 },
            in_view: true,
            player_at: Position {
                x: player_at.0,
                y: player_at.1,
            },
            noticed: true,
            pinned: false,
            swims: false,
            phasing: false,
            launcher: false,
            spellset: Vec::new(),
            roll: 0,
            helper: false,
            ally: false,
            aggravated: None,
            foes: Vec::new(),
            map,
        }
    }

    fn sighting(who: Entity, at: (u16, u16), is_player: bool) -> Sighting {
        Sighting {
            who,
            at: Position { x: at.0, y: at.1 },
            is_player,
        }
    }

    #[test]
    fn an_empty_set_waits_and_the_first_rule_that_fires_decides() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (6, 6), (7, 6));
        p.foes = vec![sighting(e[0], (7, 6), true)];
        assert_eq!(think(&p, &STILL), Action::Wait);
        assert_eq!(think(&p, &CHASER), Action::Strike(e[0]));
    }

    #[test]
    fn out_of_view_only_the_aggravated_and_the_helper_move() {
        let map = room();
        let mut p = percept(&map, (6, 6), (14, 6));
        p.in_view = false;
        assert_eq!(think(&p, &CHASER), Action::Wait);
        p.aggravated = Some(Position { x: 10, y: 4 });
        assert_eq!(think(&p, &CHASER), Action::Step(1, -1));
        p.aggravated = None;
        p.helper = true;
        assert!(matches!(think(&p, &HELPER), Action::Step(1, _)));
    }

    #[test]
    fn a_chaser_only_hunts_a_player_it_has_noticed() {
        let map = room();
        let mut p = percept(&map, (6, 6), (12, 6));
        assert_eq!(
            think(&p, &CHASER),
            Action::Wait,
            "the player is not in its foes"
        );
        let e = entities(1);
        p.foes = vec![sighting(e[0], (12, 6), true)];
        assert_eq!(think(&p, &CHASER), Action::Step(1, 0));
    }

    #[test]
    fn a_chaser_steps_around_a_wall_instead_of_into_it() {
        let mut map = room();
        map.tiles[tile_index(7, 6)] = TileType::Wall;
        let e = entities(1);
        let mut at = (5u16, 6u16);
        let think_at = |at: (u16, u16)| {
            let mut p = percept(&map, at, (9, 6));
            p.foes = vec![sighting(e[0], (9, 6), true)];
            think(&p, &CHASER)
        };
        for _ in 0..3 {
            let Action::Step(dx, dy) = think_at(at) else {
                panic!("a route around the wall exists");
            };
            at = ((at.0 as i16 + dx) as u16, (at.1 as i16 + dy) as u16);
            assert_ne!(at, (7, 6), "walked into the wall it was routing around");
        }
        assert_eq!(think_at(at), Action::Strike(e[0]));
    }

    #[test]
    fn the_player_is_struck_before_an_ally_beside_them() {
        let map = room();
        let e = entities(2);
        let mut p = percept(&map, (6, 6), (7, 6));
        p.foes = vec![sighting(e[0], (5, 6), false), sighting(e[1], (7, 6), true)];
        assert_eq!(think(&p, &AMBUSHER), Action::Strike(e[1]));
        p.foes.pop();
        assert_eq!(think(&p, &AMBUSHER), Action::Strike(e[0]));
    }

    #[test]
    fn a_helper_holds_fire_that_would_reach_the_player() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (6, 6), (5, 6));
        p.helper = true;
        p.ally = true;
        p.spellset = vec![SpellEffect::DragonBreath];
        p.foes = vec![sighting(e[0], (11, 6), false)];
        assert_eq!(
            think(&p, &HELPER),
            Action::Cast(SpellEffect::DragonBreath, Position { x: 11, y: 6 })
        );
        p.player_at = Position { x: 12, y: 6 };
        assert!(matches!(think(&p, &HELPER), Action::Step(1, 0)));
    }

    #[test]
    fn a_chaser_fires_its_spell_whoever_else_it_catches() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (6, 6), (11, 6));
        p.spellset = vec![SpellEffect::DragonBreath];
        p.foes = vec![sighting(e[0], (11, 6), true)];
        assert_eq!(
            think(&p, &CHASER),
            Action::Cast(SpellEffect::DragonBreath, Position { x: 11, y: 6 })
        );
    }

    #[test]
    fn a_spell_it_cannot_fire_sends_it_to_melee() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (5, 6), (15, 6));
        p.spellset = vec![SpellEffect::DragonBreath];
        p.foes = vec![sighting(e[0], (15, 6), true)];
        assert_eq!(think(&p, &CHASER), Action::Step(1, 0));
        p.at = Position { x: 14, y: 6 };
        p.spellset = vec![SpellEffect::Bide];
        assert_eq!(think(&p, &CHASER), Action::Strike(e[0]));
    }

    #[test]
    fn the_roll_picks_which_spell_is_tried() {
        let map = room();
        let e = entities(1);
        let mut p = percept(&map, (6, 6), (11, 6));
        p.spellset = vec![SpellEffect::DragonBreath, SpellEffect::Thunderbolt];
        p.foes = vec![sighting(e[0], (11, 6), true)];
        let at = Position { x: 11, y: 6 };
        p.roll = 0;
        assert_eq!(
            think(&p, &CHASER),
            Action::Cast(SpellEffect::DragonBreath, at)
        );
        p.roll = 1;
        assert_eq!(
            think(&p, &CHASER),
            Action::Cast(SpellEffect::Thunderbolt, at)
        );
    }
}
