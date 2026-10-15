//! `mark_moved` is the one way a creature tells the schedule it changed tile.

use bevy_ecs::prelude::*;
use models::*;

#[test]
fn a_move_is_marked_and_dirties_the_viewshed_it_leaves_behind() {
    let mut w = World::new();
    let e = w
        .spawn(Viewshed {
            visible_tiles: Vec::new(),
            revealed_tiles: fixedbitset::FixedBitSet::new(),
            range: 8,
            dirty: false,
        })
        .id();
    mark_moved(&mut w, e);
    assert!(w.get::<EntityMoved>(e).is_some());
    assert!(w.get::<Viewshed>(e).unwrap().dirty);
}

#[test]
fn a_creature_with_no_viewshed_is_still_marked() {
    let mut w = World::new();
    let e = w.spawn_empty().id();
    mark_moved(&mut w, e);
    assert!(w.get::<EntityMoved>(e).is_some());
}
