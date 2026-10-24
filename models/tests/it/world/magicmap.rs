//! A scroll of magic mapping: that reading one arms the reveal and the sweep
//! turns up every tile, that a no-op step does nothing, and that every
//! [`MagicMapStyle`] both animates and finishes with the whole floor known.

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

#[test]
fn magic_map_reveal_step_is_a_noop_when_nothing_is_armed() {
    let mut w = test_world(3);
    assert!(!magic_map_reveal_step(&mut w));
    assert!(!w.resource::<MagicMapReveal>().active);
}

#[test]
fn every_style_animates_and_reveals_the_whole_floor() {
    for style in MagicMapStyle::ALL {
        let mut w = test_world(11);
        let p = player(&mut w);
        let hero = {
            let pos = w.get::<Position>(p).unwrap();
            (pos.x, pos.y)
        };

        w.resource_mut::<MagicMapReveal>().start(hero, style);
        assert_eq!(w.resource::<MagicMapReveal>().style, style);

        let mut frames = 0;
        while magic_map_reveal_step(&mut w) {
            frames += 1;
            assert!(frames < MAP_TILE_COUNT, "{style:?} must terminate");
        }

        assert!(
            !w.resource::<MagicMapReveal>().active,
            "{style:?} left the reveal armed"
        );
        let vs = w.get::<Viewshed>(p).unwrap();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                assert!(
                    vs.revealed_tiles.contains(tile_index(x, y)),
                    "{style:?}: tile ({x},{y}) not revealed"
                );
            }
        }
    }
}

#[test]
fn style_names_and_flavour_lines_are_distinct() {
    assert_eq!(
        MagicMapStyle::from_name("rows"),
        Some(MagicMapStyle::RowByRow)
    );
    assert_eq!(
        MagicMapStyle::from_name(" Spiral "),
        Some(MagicMapStyle::Spiral)
    );
    assert_eq!(
        MagicMapStyle::from_name("BLAST"),
        Some(MagicMapStyle::Explode)
    );
    assert_eq!(MagicMapStyle::from_name("nonsense"), None);

    let lines: Vec<&str> = MagicMapStyle::ALL.iter().map(|s| s.flavour()).collect();
    assert!(
        lines.iter().collect::<std::collections::HashSet<_>>().len() == lines.len(),
        "flavour lines must differ"
    );
}
