//! The Mortal-Kombat-style death flourish: a flung corpse, scattering bone
//! shrapnel, and — with blood switched off — the quiet fallback of a static
//! corpse mark and no animation at all.

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<UseQueue>();
    w.init_resource::<PlayerTempo>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn spawn_dummy(w: &mut World, name: &str, x: u16, y: u16, hp: i32) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Mob {
            movement_type: MovementType::Static,
        },
        Position { x, y },
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
    ))
    .id()
}

/// A tile anywhere on the map has a corpse mark.
fn any_corpse(w: &World) -> bool {
    let corpses = w.resource::<Corpses>();
    (0..MAP_WIDTH).any(|x| (0..MAP_HEIGHT).any(|y| corpses.has(x, y)))
}

/// Wallpapers the whole floor as open room tiles, so a blast's line-of-sight
/// and a corpse's flight path never trip over the generated dungeon's own
/// walls — the two things this file wants deterministic, not the map.
fn open_map(w: &mut World) {
    let count = (MAP_WIDTH as usize) * (MAP_HEIGHT as usize);
    w.insert_resource(Map {
        tiles: vec![TileType::Room; count],
        dark: FixedBitSet::with_capacity(count),
    });
}

/// A fresh, well-charged wand of `effect`, already in `p`'s pack.
fn give_wand(w: &mut World, p: Entity, effect: WandEffect) -> Entity {
    let wand = spawn_wand(w, effect, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Battery>(wand).unwrap().charges = 9;
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    wand
}

/// Zap `wand` at `target` — the engine's targeted "Use".
fn zap(w: &mut World, user: Entity, wand: Entity, target: Position) {
    let i = w
        .get_mut::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == wand);
    if let Some(i) = i {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
        w.resource_mut::<UseQueue>().uses.push(WantsToUse {
            user,
            item: wand,
            target: Some(target),
            slot_idx: Some(i),
        });
    }
    item_system(w);
}

#[test]
fn a_lethal_hit_flings_a_corpse_and_scatters_bones() {
    let mut w = test_world(1);
    w.insert_resource(Particles::new());
    let p = player(&mut w);
    // Overwhelming power, no armour on the target: the very first swing kills.
    w.get_mut::<Fighter>(p).unwrap().power = 100;
    let hero = *w.get::<Position>(p).unwrap();
    let rat = spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 1);

    resolve_attack(&mut w, p, rat);

    assert!(!w.entities().contains(rat), "the rat is dead and despawned");
    assert!(any_corpse(&w), "the burst leaves a corpse mark somewhere");

    let fx = w.resource::<Particles>();
    assert!(
        fx.pending,
        "a lethal hit queues the death burst's particles"
    );
    assert!(
        fx.live
            .iter()
            .any(|p| p.frames.iter().any(|&(g, _)| g == '%')),
        "the flung corpse itself is queued"
    );
    assert!(
        fx.live.iter().any(|p| p
            .frames
            .iter()
            .any(|&(g, _)| matches!(g, '/' | '\\' | '|' | '¡'))),
        "bone shrapnel is queued alongside the corpse"
    );
}

#[test]
fn no_blood_mode_skips_the_animation_and_just_leaves_a_corpse() {
    let mut w = test_world(2);
    w.resource_mut::<BloodStains>().enabled = false;
    w.insert_resource(Particles::new());
    let p = player(&mut w);
    w.get_mut::<Fighter>(p).unwrap().power = 100;
    let hero = *w.get::<Position>(p).unwrap();
    let rat = spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 1);

    resolve_attack(&mut w, p, rat);

    assert!(!w.entities().contains(rat), "the rat is dead and despawned");
    assert!(
        w.resource::<Corpses>().has(hero.x + 1, hero.y),
        "with no animation to fling it, the corpse sits right where it died"
    );
    // The hit still pops its ordinary (blood-independent) impact spark; what
    // -nb skips is the corpse-and-bones burst specifically.
    assert!(
        !w.resource::<Particles>()
            .live
            .iter()
            .any(|p| p.frames.iter().any(|&(g, _)| g == '%')),
        "-nb spends no animation (and no RNG) on the kill"
    );
}

#[test]
fn an_indirect_kill_still_bursts_via_the_reaper() {
    let mut w = test_world(3);
    w.insert_resource(Particles::new());
    let p = player(&mut w);
    let hero = *w.get::<Position>(p).unwrap();
    let rat = spawn_dummy(&mut w, "rat", hero.x + 1, hero.y, 1);

    // No attacker entity survives a wand bolt or a blast — the reaper is what
    // sweeps these up, with no source position to fling the corpse away from.
    w.get_mut::<Fighter>(rat).unwrap().hp = 0;
    reaper_system(&mut w);

    assert!(
        !w.entities().contains(rat),
        "the rat is swept up by the reaper"
    );
    assert!(
        any_corpse(&w),
        "the reaper's kill still leaves a corpse mark"
    );
    assert!(
        w.resource::<Particles>().pending,
        "the reaper's kill still bursts, just with a random fling direction"
    );
}

#[test]
fn an_explosion_flings_its_edge_casualties_radially_outward() {
    let mut w = test_world(4);
    open_map(&mut w);
    w.insert_resource(Particles::new());

    let p = player(&mut w);
    // Well clear of the blast so the player isn't a casualty too.
    w.get_mut::<Position>(p).unwrap().x = 5;
    w.get_mut::<Position>(p).unwrap().y = 5;

    // One casualty sits dead centre on the blast (no defined "away from
    // centre" direction — it gets a random fling, same as any other indirect
    // kill); the other sits one tile off to the east, so its corpse has a
    // real radial direction to fly in: straight on outward, away from centre.
    let center = Position { x: 30, y: 10 };
    let edge = Position { x: 31, y: 10 };
    let ground_zero = spawn_dummy(&mut w, "rat", center.x, center.y, 1);
    let rim = spawn_dummy(&mut w, "rat", edge.x, edge.y, 1);

    let wand = give_wand(&mut w, p, WandEffect::Fire);
    zap(&mut w, p, wand, center);

    assert!(
        !w.entities().contains(ground_zero),
        "the blast's own damage (3d3, armourless 1 HP) is always lethal"
    );
    assert!(!w.entities().contains(rim), "same for the off-centre rat");

    // The off-centre rat's corpse flies straight on past its own tile, away
    // from the blast's centre — never back toward it, never off to a side.
    let corpses = w.resource::<Corpses>();
    assert!(
        (32..=35).any(|x| corpses.has(x, edge.y)),
        "the edge casualty's corpse lands further east, continuing outward \
         from the blast centre through where it stood"
    );
    assert!(
        !(0..=31).any(|x| corpses.has(x, edge.y)),
        "and never west of where it stood — that would be back toward the blast"
    );
}

#[test]
fn the_players_glyph_blanks_the_instant_they_die() {
    let mut w = test_world(5);
    w.insert_resource(Particles::new());
    let p = player(&mut w);

    let hero = *w.get::<Position>(p).unwrap();
    let ogre = spawn_dummy(&mut w, "ogre", hero.x, hero.y, 20);
    w.get_mut::<Fighter>(ogre).unwrap().power = 999;

    resolve_attack(&mut w, ogre, p);

    assert!(w.resource::<Ending>().player_dead, "the blow was lethal");
    assert_eq!(
        w.get::<Renderable>(p).unwrap().glyph,
        ' ',
        "the player's own glyph is blanked so the flung corpse reads as them \
         exploding, not detaching from a body still standing there"
    );
}
