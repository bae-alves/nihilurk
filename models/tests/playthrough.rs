use bevy_ecs::prelude::*;
use models::*;

/// Headless stand-in for the engine loop: drive a few floors, pick everything
/// up, read/quaff the consumables, and make sure nothing panics and the drop
/// table actually hands out one of every category over a short run.
#[test]
fn walk_a_few_floors_and_use_the_loot() {
    let mut w = World::new();
    // Seed picked so a short 8-floor run turns up at least one of every loot
    // category (rings are only 5% of drops, so this is deliberately calibrated —
    // re-pick it if the content stream shifts, e.g. new spawn logic or a change
    // to how `content_rng` is keyed).
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(7)));
    w.insert_resource(RngSeed(7));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);

    let mut seen_weapon = false;
    let mut seen_armor = false;
    let mut seen_ring = false;
    let mut used_consumables = 0;

    for _ in 0..8 {
        // Scoop every loose floor item into the backpack.
        let floor: Vec<Entity> = w
            .query_filtered::<Entity, (With<Item>, With<Position>)>()
            .iter(&w)
            .filter(|e| w.get::<Position>(*e).is_some())
            .collect();
        for e in floor {
            // The stale-positioned starting wand is already in the pack; skip it.
            let in_pack = w
                .get::<Backpack>(player)
                .map(|b| b.items.contains(&e))
                .unwrap_or(false);
            if in_pack {
                continue;
            }
            seen_weapon |= w.get::<PowerDie>(e).is_some();
            seen_armor |= w.get::<ArmorDie>(e).is_some();
            seen_ring |= w.get::<Ring>(e).is_some();
            w.entity_mut(e).remove::<Position>();
            w.get_mut::<Backpack>(player).unwrap().items.push(e);
        }

        // Read / drink anything consumable.
        let consumables: Vec<Entity> = w
            .get::<Backpack>(player)
            .unwrap()
            .items
            .iter()
            .copied()
            .filter(|e| w.get::<Consume>(*e).is_some())
            .collect();
        for item in consumables {
            w.get_mut::<Backpack>(player)
                .unwrap()
                .items
                .retain(|&x| x != item);
            w.resource_mut::<UseQueue>().uses.push(WantsToUse {
                user: player,
                item,
                target: None,
                slot_idx: None,
            });
            item_system(&mut w);
            used_consumables += 1;
        }

        // Descend.
        let down = w
            .resource::<Map>()
            .tiles
            .iter()
            .position(|&t| t == TileType::Downstairs)
            .unwrap();
        w.get_mut::<Position>(player).unwrap().x = (down % MAP_WIDTH as usize) as u16;
        w.get_mut::<Position>(player).unwrap().y = (down / MAP_WIDTH as usize) as u16;
        assert!(change_level(&mut w, true));
    }

    assert_eq!(w.resource::<Depth>().what, 9);
    assert!(
        used_consumables > 0,
        "never found a scroll or potion in 8 floors"
    );
    assert!(seen_weapon, "never found a weapon in 8 floors");
    assert!(seen_armor, "never found armor in 8 floors");
    assert!(seen_ring, "never found a ring in 8 floors");

    // Player still alive and coherent.
    let f = w.get::<Fighter>(player).unwrap();
    assert!(f.hp <= f.max_hp);
}
