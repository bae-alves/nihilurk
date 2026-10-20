use super::*;
use crate::catalog::{BIDE, FIREBALL, FORCE_LANCE, SpellDef, THUNDERBOLT, spell_row};
use crate::map::{MAP_TILE_COUNT, TileType, tile_index};
use bevy_ecs::world::World;
use fixedbitset::FixedBitSet;

/// Walls everywhere, with one open room at x 5..=15, y 4..=8.
fn room() -> Map {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: FixedBitSet::with_capacity(MAP_TILE_COUNT),
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
fn a_chaser_goes_for_the_nearer_ally_over_a_farther_player() {
    let map = room();
    let e = entities(2);
    let mut p = percept(&map, (10, 6), (14, 6));
    p.foes = vec![
        sighting(e[0], (10, 4), false),
        sighting(e[1], (14, 6), true),
    ];
    assert_eq!(think(&p, &CHASER), Action::Step(0, -1));
}

#[test]
fn a_charmed_ally_closes_in_on_the_nearest_monster() {
    let map = room();
    let e = entities(1);
    let mut p = percept(&map, (6, 6), (6, 5));
    p.ally = true;
    p.foes = vec![sighting(e[0], (12, 6), false)];
    let set = rule_set_for(MovementType::Chase, false, p.ally);
    assert_eq!(think(&p, set), Action::Step(1, 0));
    p.at = Position { x: 11, y: 6 };
    assert_eq!(think(&p, set), Action::Strike(e[0]));
}

#[test]
fn a_charmed_ally_with_nothing_to_fight_stays_put() {
    let map = room();
    let mut p = percept(&map, (6, 6), (14, 6));
    p.ally = true;
    let set = rule_set_for(MovementType::Chase, false, p.ally);
    assert_eq!(think(&p, set), Action::Wait);
}

#[test]
fn a_helper_keeps_its_own_set_over_the_ally_one() {
    assert!(std::ptr::eq(
        rule_set_for(MovementType::Chase, true, true),
        &HELPER
    ));
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
    p.spellset = vec![&FIREBALL];
    p.foes = vec![sighting(e[0], (11, 6), false)];
    assert_eq!(
        think(&p, &HELPER),
        Action::Cast(&FIREBALL, Position { x: 11, y: 6 })
    );
    p.player_at = Position { x: 12, y: 6 };
    assert!(matches!(think(&p, &HELPER), Action::Step(1, 0)));
}

#[test]
fn a_targeted_spell_needs_no_clear_line() {
    let mut map = room();
    map.tiles[tile_index(9, 6)] = TileType::Wall;
    let e = entities(1);
    for spell in [spell_row("Sting").unwrap(), &THUNDERBOLT] {
        let mut p = percept(&map, (6, 6), (11, 6));
        p.spellset = vec![spell];
        p.foes = vec![sighting(e[0], (11, 6), true)];
        assert_eq!(
            think(&p, &CHASER),
            Action::Cast(spell, Position { x: 11, y: 6 })
        );
    }
}

#[test]
fn a_line_spell_needs_a_clear_line() {
    let mut map = room();
    map.tiles[tile_index(9, 6)] = TileType::Wall;
    let e = entities(1);
    let mut p = percept(&map, (6, 6), (11, 6));
    p.spellset = vec![&FORCE_LANCE];
    p.foes = vec![sighting(e[0], (11, 6), true)];
    assert!(!matches!(think(&p, &CHASER), Action::Cast(..)));
}

#[test]
fn a_helper_casts_every_attack_spell_that_spares_the_player() {
    let map = room();
    let e = entities(1);
    let foe = Position { x: 11, y: 6 };
    for spell in [
        spell_row("Sting").unwrap(),
        &THUNDERBOLT,
        &FORCE_LANCE,
        spell_row("Lux").unwrap(),
        spell_row("Meteor Strike").unwrap(),
    ] {
        let mut p = percept(&map, (6, 6), (5, 8));
        p.helper = true;
        p.ally = true;
        p.spellset = vec![spell];
        p.foes = vec![sighting(e[0], (11, 6), false)];
        assert_eq!(think(&p, &HELPER), Action::Cast(spell, foe), "{spell:?}");
    }
}

#[test]
fn a_helper_holds_a_lance_through_the_player_but_not_a_bolt_beside_them() {
    let map = room();
    let e = entities(1);
    let mut p = percept(&map, (6, 6), (8, 6));
    p.helper = true;
    p.ally = true;
    p.foes = vec![sighting(e[0], (11, 6), false)];
    p.spellset = vec![&FORCE_LANCE];
    assert!(!matches!(think(&p, &HELPER), Action::Cast(..)));
    p.spellset = vec![&THUNDERBOLT];
    assert_eq!(
        think(&p, &HELPER),
        Action::Cast(&THUNDERBOLT, Position { x: 11, y: 6 })
    );
}

static BOLT: SpellDef = SpellDef {
    kind: SpellKind::Attack,
    range: 8,
    ..SpellDef::new("bolt of the test", 1, |_, _| {})
};

static BURST: SpellDef = SpellDef {
    blast: Some(1.5),
    ..BOLT
};

static CHAIN: SpellDef = SpellDef { spread: 3, ..BURST };

static LANCE: SpellDef = SpellDef { line: true, ..BOLT };

/// A helper aiming at a foe at (11, 6) from (6, 6), with the player at `player_at`.
fn helper_casts(map: &Map, spell: &'static SpellDef, player_at: (u16, u16)) -> bool {
    let e = entities(1);
    let mut p = percept(map, (6, 6), player_at);
    p.helper = true;
    p.ally = true;
    p.spellset = vec![spell];
    p.foes = vec![sighting(e[0], (11, 6), false)];
    think(&p, &HELPER) == Action::Cast(spell, Position { x: 11, y: 6 })
}

#[test]
fn a_helper_keeps_the_player_out_of_a_rows_blast_and_its_spread() {
    let map = room();
    assert!(helper_casts(&map, &BOLT, (12, 6)));
    assert!(!helper_casts(&map, &BURST, (12, 6)));
    assert!(helper_casts(&map, &BURST, (14, 6)));
    assert!(!helper_casts(&map, &CHAIN, (14, 6)));
}

#[test]
fn a_helper_keeps_the_player_off_a_rows_line() {
    let map = room();
    assert!(helper_casts(&map, &BOLT, (8, 6)));
    assert!(!helper_casts(&map, &LANCE, (8, 6)));
    let mut walled = room();
    walled.tiles[tile_index(9, 6)] = TileType::Wall;
    assert!(helper_casts(&walled, &BOLT, (5, 8)));
    assert!(!helper_casts(&walled, &LANCE, (5, 8)));
}

#[test]
fn a_chaser_fires_its_spell_whoever_else_it_catches() {
    let map = room();
    let e = entities(1);
    let mut p = percept(&map, (6, 6), (11, 6));
    p.spellset = vec![&FIREBALL];
    p.foes = vec![sighting(e[0], (11, 6), true)];
    assert_eq!(
        think(&p, &CHASER),
        Action::Cast(&FIREBALL, Position { x: 11, y: 6 })
    );
}

#[test]
fn a_spell_it_cannot_fire_sends_it_to_melee() {
    let map = room();
    let e = entities(1);
    let mut p = percept(&map, (5, 6), (15, 6));
    p.spellset = vec![&FIREBALL];
    p.foes = vec![sighting(e[0], (15, 6), true)];
    assert_eq!(think(&p, &CHASER), Action::Step(1, 0));
    p.at = Position { x: 14, y: 6 };
    p.spellset = vec![&BIDE];
    assert_eq!(think(&p, &CHASER), Action::Strike(e[0]));
}

#[test]
fn the_roll_picks_which_spell_is_tried() {
    let map = room();
    let e = entities(1);
    let mut p = percept(&map, (6, 6), (11, 6));
    p.spellset = vec![&FIREBALL, &THUNDERBOLT];
    p.foes = vec![sighting(e[0], (11, 6), true)];
    let at = Position { x: 11, y: 6 };
    p.roll = 0;
    assert_eq!(think(&p, &CHASER), Action::Cast(&FIREBALL, at));
    p.roll = 1;
    assert_eq!(think(&p, &CHASER), Action::Cast(&THUNDERBOLT, at));
}

#[test]
fn nothing_swings_shoots_or_casts_across_a_doorway_corner() {
    let mut map = room();
    map.tiles[tile_index(16, 5)] = TileType::Door;
    let e = entities(1);
    let mut p = percept(&map, (15, 4), (16, 5));
    p.launcher = true;
    p.spellset = vec![&FIREBALL];
    p.foes = vec![sighting(e[0], (16, 5), true)];
    assert!(!matches!(
        think(&p, &CHASER),
        Action::Strike(_) | Action::Shoot(_) | Action::Cast(..)
    ));
}
