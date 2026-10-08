//! Shared scaffolding for the integration tests.
//!
//! Cargo compiles every *file* under `tests/` as its own test binary, and
//! `tests/common/` is the one directory it treats as a plain module instead —
//! so this is where a helper goes when it must not become a test target of its
//! own.

use std::path::PathBuf;

/// A save file with a path nothing else will pick, that deletes itself.
///
/// Two things used to be wrong with the eleven save round-trip tests. They
/// named fixed files in the shared temp directory (`nihilurk_test.sav`,
/// `nihilurk_melee_cap.sav`, …), so two `cargo test` runs at once — or a CI matrix
/// sharing one `/tmp` — could write the same path from different tests and see
/// each other's bytes. And cleanup was by hand, which meant two of them never
/// did it: a stale save in `/tmp` is how a test starts passing for the wrong
/// reason the next time the save format changes.
///
/// The process id in the name fixes the first, and `Drop` fixes the second by
/// making it impossible to forget.
#[allow(dead_code)]
pub struct SaveFile(PathBuf);

#[allow(dead_code)]
impl SaveFile {
    /// `tag` only has to be unique within one test binary; the pid separates
    /// the binaries and the runs.
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
