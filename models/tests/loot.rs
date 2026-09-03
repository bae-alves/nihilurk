use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    w
}

fn descend(w: &mut World) {
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    let p = w.query_filtered::<Entity, With<Player>>().single(w);
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(w, true));
}

#[derive(Default, Debug)]
struct Tally {
    scrolls: u32,
    potions: u32,
    coins: u32,
    armor: u32,
    /// The whole armoury: melee weapons, ammunition and launchers alike.
    weapons: u32,
    wands: u32,
    rings: u32,
    total: u32,
    // The armoury's own three-way split, counted inside `weapons`.
    melee: u32,
    ammo: u32,
    launchers: u32,
    /// Arrows and quarrels actually spawned, counting each bundle's contents.
    arrows: u32,
}

#[test]
fn floor_loot_follows_the_rogue_drop_table() {
    let mut t = Tally::default();

    for seed in 0..600u64 {
        let mut w = test_world(seed);
        for _ in 0..4 {
            // The player's starting wand keeps a stale Position while sitting in
            // the backpack, so exclude backpack contents explicitly.
            let carried: std::collections::HashSet<Entity> = w
                .query_filtered::<&Backpack, With<Player>>()
                .single(&w)
                .items
                .iter()
                .copied()
                .collect();
            let mut q = w.query_filtered::<
                (
                    Entity,
                    Option<&Potion>,
                    Option<&Scroll>,
                    Option<&Wand>,
                    Option<&ArmorDie>,
                    Option<&PowerDie>,
                    Option<&Ring>,
                    Option<&Value>,
                    Option<&Stack>,
                    Option<&Launcher>,
                ),
                (With<Item>, With<Position>),
            >();
            for (e, potion, scroll, wand, armor, weapon, ring, value, stack, launcher) in q.iter(&w)
            {
                if carried.contains(&e) {
                    continue;
                }
                t.total += 1;
                if scroll.is_some() {
                    t.scrolls += 1;
                } else if potion.is_some() {
                    t.potions += 1;
                } else if value.is_some() {
                    t.coins += 1;
                } else if armor.is_some() {
                    t.armor += 1;
                } else if weapon.is_some() {
                    t.weapons += 1;
                    t.melee += 1;
                } else if let Some(stack) = stack {
                    // Ammunition: one drop, several arrows.
                    t.weapons += 1;
                    t.ammo += 1;
                    t.arrows += stack.count as u32;
                } else if launcher.is_some() {
                    t.weapons += 1;
                    t.launchers += 1;
                } else if wand.is_some() {
                    t.wands += 1;
                } else if ring.is_some() {
                    t.rings += 1;
                } else {
                    panic!("floor item with no recognised category component");
                }
            }
            descend(&mut w);
        }
    }

    // Every category shows up.
    assert!(t.scrolls > 0 && t.potions > 0 && t.coins > 0);
    assert!(t.armor > 0 && t.weapons > 0 && t.wands > 0 && t.rings > 0);
    assert!(t.melee > 0 && t.ammo > 0 && t.launchers > 0, "armoury gap: {t:?}");

    // Proportions land near the drop table (generous tolerance for sampling).
    let pct = |n: u32| 100.0 * n as f64 / t.total as f64;
    let near = |got: f64, want: f64| {
        assert!(
            (got - want).abs() < 4.0,
            "category share {got:.1}% too far from target {want:.1}% (tally: {t:?})"
        );
    };
    near(pct(t.scrolls), 30.0);
    near(pct(t.potions), 27.0);
    near(pct(t.coins), 17.0);
    near(pct(t.armor), 8.0);
    near(pct(t.weapons), 8.0);
    near(pct(t.wands), 5.0);
    near(pct(t.rings), 5.0);

    // Inside the weapon share, the armoury's own 45 / 35 / 20 split.
    let arm_pct = |n: u32| 100.0 * n as f64 / t.weapons as f64;
    let near_arm = |got: f64, want: f64| {
        assert!(
            (got - want).abs() < 6.0,
            "armoury share {got:.1}% too far from target {want:.1}% (tally: {t:?})"
        );
    };
    near_arm(arm_pct(t.melee), 45.0);
    near_arm(arm_pct(t.ammo), 35.0);
    near_arm(arm_pct(t.launchers), 20.0);

    // A drop of ammunition is a bundle, never a lone arrow: 3-12 a time.
    let per_bundle = t.arrows as f64 / t.ammo as f64;
    assert!(
        (3.0..=12.0).contains(&per_bundle),
        "a bundle averaged {per_bundle:.1} arrows, outside 3..=12 (tally: {t:?})"
    );
}
