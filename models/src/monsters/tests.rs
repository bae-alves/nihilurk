use super::*;

#[test]
fn the_ghost_phases() {
    let mut w = World::new();
    let ghost = spawn_monster(&mut w, &GHOST, Position { x: 1, y: 1 });
    let phases = Grant::of::<Phasing>().probe(&w, ghost);
    assert!(phases);
}
