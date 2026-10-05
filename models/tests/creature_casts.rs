//! Every spell a creature can be born knowing, cast by that creature, at every
//! kind of tile it might aim at. A spell that only ever ran from the player's
//! side can assume a player caster; this sweep is what catches it.

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Open floor tiles around the player, nearest first.
fn open_tiles_near(w: &World, at: Position, n: usize) -> Vec<Position> {
    let map = w.resource::<Map>();
    let mut out = Vec::new();
    for r in 1i32..6 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let (x, y) = (at.x as i32 + dx, at.y as i32 + dy);
                if x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16) {
                    out.push(Position {
                        x: x as u16,
                        y: y as u16,
                    });
                    if out.len() == n {
                        return out;
                    }
                }
            }
        }
    }
    out
}

fn wall_near(w: &World, at: Position) -> Position {
    let map = w.resource::<Map>();
    for r in 1i32..20 {
        for dx in -r..=r {
            let (x, y) = (at.x as i32 + dx, at.y as i32 - r);
            if x >= 0 && y >= 0 && map.blocks(x as u16, y as u16) {
                return Position {
                    x: x as u16,
                    y: y as u16,
                };
            }
        }
    }
    Position { x: 0, y: 0 }
}

#[test]
fn every_creature_can_cast_every_spell_it_knows_at_anything() {
    for def in BESTIARY.iter().filter(|d| !d.spells.is_empty()) {
        for &spell in def.spells {
            for seed in 0..4u64 {
                for aim in 0..4 {
                    let mut w = test_world(seed);
                    let p = player(&mut w);
                    let here = *w.get::<Position>(p).unwrap();
                    let tiles = open_tiles_near(&w, here, 3);
                    let caster = spawn_monster(&mut w, def, tiles[0]);
                    let bystander = spawn_monster(&mut w, MonsterDef::named("orc"), tiles[1]);
                    w.entity_mut(bystander).insert(Faction::Ally);
                    let target = match aim {
                        0 => here,
                        1 => tiles[1],
                        2 => tiles[2],
                        _ => wall_near(&w, here),
                    };
                    w.resource_mut::<SpellQueue>().spells.push(WantsToCast {
                        user: caster,
                        effect: spell,
                        target,
                    });
                    spell_system(&mut w);
                }
            }
        }
    }
}

/// Every spell has its row in `SPELLS`. 0.1.3 shipped a creature spell with no
/// row, and the first gnome to cast it panicked the game. The match has no
/// catch-all, so a new spell fails to compile here until it is listed.
#[test]
fn every_spell_has_a_catalog_row() {
    fn listed(e: SpellEffect) -> SpellEffect {
        match e {
            SpellEffect::DragonBreath
            | SpellEffect::Sting
            | SpellEffect::Thunderbolt
            | SpellEffect::Cure
            | SpellEffect::Bide
            | SpellEffect::ForceLance
            | SpellEffect::Identify
            | SpellEffect::Setup
            | SpellEffect::Lux
            | SpellEffect::CircleOfDeath
            | SpellEffect::MagicWard
            | SpellEffect::Heal
            | SpellEffect::MeteorStrike
            | SpellEffect::FrostNova
            | SpellEffect::MagicMapping
            | SpellEffect::HasteSelf
            | SpellEffect::PolymorphSelf
            | SpellEffect::PolymorphOther
            | SpellEffect::GateDown => e,
        }
    }
    let all = [
        SpellEffect::DragonBreath,
        SpellEffect::Sting,
        SpellEffect::Thunderbolt,
        SpellEffect::Cure,
        SpellEffect::Bide,
        SpellEffect::ForceLance,
        SpellEffect::Identify,
        SpellEffect::Setup,
        SpellEffect::Lux,
        SpellEffect::CircleOfDeath,
        SpellEffect::MagicWard,
        SpellEffect::Heal,
        SpellEffect::MeteorStrike,
        SpellEffect::FrostNova,
        SpellEffect::MagicMapping,
        SpellEffect::HasteSelf,
        SpellEffect::PolymorphSelf,
        SpellEffect::PolymorphOther,
        SpellEffect::GateDown,
    ];
    for e in all {
        assert!(SPELLS.iter().any(|row| row.effect == listed(e)), "{e:?}");
    }
    assert_eq!(SPELLS.len(), all.len(), "a row for a spell not listed here");
}
