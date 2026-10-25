//! Shared scaffolding for the integration tests.
//!
//! `main.rs` declares it once and every test module reaches it as
//! `crate::common`. `tests/effects.rs`, a binary of its own, borrows
//! `monster.rs` by path.

pub mod fixtures;
pub mod monster;

use std::path::PathBuf;

/// A save file with a path nothing else will pick, that deletes itself.
///
/// A fixed name in the shared temp directory lets two `cargo test` runs at
/// once — or a CI matrix sharing one `/tmp` — see each other's bytes, so the
/// name carries the process id. Cleanup by hand gets forgotten, and a stale
/// save in `/tmp` is how a test starts passing for the wrong reason, so
/// cleanup lives in `Drop`.
#[allow(dead_code)]
pub struct SaveFile(PathBuf);

#[allow(dead_code)]
impl SaveFile {
    /// `tag` has to be unique across `tests/it`, which is one binary; the pid
    /// separates the runs.
    pub fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!("nihilurk-{}-{tag}.sav", std::process::id())))
    }

    /// The path, as `save_game` and `load_game` want it.
    pub fn path(&self) -> &str {
        self.0.to_str().expect("the temp directory is valid utf-8")
    }
}

impl Drop for SaveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A bare world holding a generated first floor, every resource the turn
/// schedule reads, and the player's entity. Monsters and items are left where
/// the generator put them.
#[allow(dead_code)]
pub fn fresh_run(seed: u64) -> (bevy_ecs::world::World, bevy_ecs::entity::Entity) {
    use bevy_ecs::prelude::*;
    use models::*;

    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.init_resource::<Ending>();
    w.init_resource::<DungeonLord>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<PlayerActionQueue>();
    w.init_resource::<BarterMenu>();
    w.init_resource::<OfferMenu>();
    w.init_resource::<ExtraMonsterRound>();
    w.init_resource::<PlayerTempo>();
    initialize_world(&mut w);
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    (w, player)
}
