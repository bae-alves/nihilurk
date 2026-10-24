//! The run dump: the kill tally it reads, and the text file it writes when a
//! run ends.

use crate::common::monster;

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let strays: Vec<Entity> = w
        .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
        .iter(&w)
        .collect();
    for e in strays {
        w.despawn(e);
    }
    w
}

fn kill(w: &mut World, name: &str) {
    let victim = monster::plain_monster(w, name, Position { x: 1, y: 1 });
    w.get_mut::<Fighter>(victim).unwrap().hp = 0;
    reaper_system(w);
}

fn tally(w: &World) -> Vec<(String, u32)> {
    w.resource::<Kills>()
        .by_name
        .iter()
        .map(|(n, c)| (n.clone(), *c))
        .collect()
}

#[test]
fn every_corpse_is_counted_under_the_name_it_died_with() {
    let mut w = test_world(1);
    kill(&mut w, "fixture-gnat");
    kill(&mut w, "fixture-gnat");
    kill(&mut w, "fixture-moth");
    assert_eq!(
        tally(&w),
        vec![("fixture-gnat".into(), 2), ("fixture-moth".into(), 1)]
    );
}
