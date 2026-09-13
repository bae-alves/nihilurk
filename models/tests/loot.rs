use bevy_ecs::prelude::*;
use models::constants::loot::{AMMO_BUNDLE_MAX, AMMO_BUNDLE_MIN};
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
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
            let mut q = w.query_filtered::<(
                Entity,
                Option<&Potion>,
                Option<&Scroll>,
                Option<&Wand>,
                Option<&ArmorDie>,
                Option<&PowerDie>,
                Option<&Ring>,
                Option<&Pickup>,
                Option<&Stack>,
                Option<&Launcher>,
                Option<&Equipped>,
            ), (With<Item>, With<Position>)>();
            for (e, potion, scroll, wand, armor, weapon, ring, pickup, stack, launcher, equipped) in
                q.iter(&w)
            {
                if carried.contains(&e) {
                    continue;
                }
                // A monster's starting gear (a centaur's bow, a hobgoblin's
                // sword) is bestiary-driven, not a roll of `DROPS` — it would
                // skew the very table this test is checking.
                if equipped.is_some_and(|eq| eq.by.is_some()) {
                    continue;
                }
                t.total += 1;
                if scroll.is_some() {
                    t.scrolls += 1;
                } else if potion.is_some() {
                    t.potions += 1;
                } else if pickup.is_some() {
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
    assert!(
        t.melee > 0 && t.ammo > 0 && t.launchers > 0,
        "armoury gap: {t:?}"
    );

    // Proportions land near the drop table — and "the drop table" means the
    // weights in `DROPS`, read here rather than copied. What this proves is that
    // the roller honours the weights it is given; what those weights *are* is
    // somebody's balance decision and no business of a test's.
    let share_of = |name: &str| {
        let total: u32 = DROPS.iter().map(|c| c.weight).sum();
        let want = DROPS.iter().find(|c| c.name == name).expect(name).weight;
        100.0 * want as f64 / total as f64
    };
    let pct = |n: u32| 100.0 * n as f64 / t.total as f64;
    let near = |got: f64, want: f64| {
        assert!(
            (got - want).abs() < 4.0,
            "category share {got:.1}% too far from the table's {want:.1}% (tally: {t:?})"
        );
    };
    near(pct(t.scrolls), share_of("scroll"));
    near(pct(t.potions), share_of("potion"));
    near(pct(t.coins), share_of("coin"));
    near(pct(t.armor), share_of("armor"));
    near(pct(t.wands), share_of("wand"));
    near(pct(t.rings), share_of("ring"));
    near(
        pct(t.weapons),
        share_of("weapon") + share_of("ammo") + share_of("launcher"),
    );

    // And inside the armoury, the same test of the same property: three
    // categories in the proportions the table asked for.
    let armoury = share_of("weapon") + share_of("ammo") + share_of("launcher");
    let arm_pct = |n: u32| 100.0 * n as f64 / t.weapons as f64;
    let near_arm = |got: f64, want: f64| {
        assert!(
            (got - want).abs() < 6.0,
            "armoury share {got:.1}% too far from the table's {want:.1}% (tally: {t:?})"
        );
    };
    near_arm(arm_pct(t.melee), 100.0 * share_of("weapon") / armoury);
    near_arm(arm_pct(t.ammo), 100.0 * share_of("ammo") / armoury);
    near_arm(arm_pct(t.launchers), 100.0 * share_of("launcher") / armoury);

    // A drop of ammunition is a bundle, never a lone arrow.
    let per_bundle = t.arrows as f64 / t.ammo as f64;
    let (min, max) = (AMMO_BUNDLE_MIN as f64, AMMO_BUNDLE_MAX as f64);
    assert!(
        (min..=max).contains(&per_bundle),
        "a bundle averaged {per_bundle:.1} arrows, outside {min}..={max} (tally: {t:?})"
    );
}

/// Every loose item on the current floor, excluding the player's starting kit
/// (which keeps a stale [`Position`] while sitting in the backpack).
fn floor_item_count(w: &mut World) -> usize {
    let carried: std::collections::HashSet<Entity> = w
        .query_filtered::<&Backpack, With<Player>>()
        .single(w)
        .items
        .iter()
        .copied()
        .collect();
    w.query_filtered::<(Entity, Option<&Equipped>), (With<Item>, With<Position>)>()
        .iter(w)
        .filter(|(e, eq)| !carried.contains(e) && !eq.is_some_and(|eq| eq.by.is_some()))
        .count()
}

#[test]
fn the_item_budget_scales_with_depth() {
    // Items are `3 + tier` attempts, so depth-13 floors (tier 4) run seven
    // attempts to a surface floor's three. Sampled across seeds, the deep
    // floors should carry noticeably more loot.
    let mut shallow = 0usize;
    let mut deep = 0usize;

    for seed in 0..40u64 {
        let mut w = test_world(seed);
        shallow += floor_item_count(&mut w);
        while w.resource::<Depth>().what < 13 {
            descend(&mut w);
        }
        deep += floor_item_count(&mut w);
    }

    assert!(
        deep > shallow + shallow / 2,
        "deep floors ({deep}) should out-loot shallow ones ({shallow}) by half again"
    );
}

#[test]
fn a_stashed_item_is_invisible_and_hidden_together() {
    // The rate the dungeon stashes at is `population::HIDDEN_ITEM_CHANCE` and
    // belongs to whoever is balancing the game, not to a test. What must hold
    // whatever it is set to: a stash is always *both* tags, because the two do
    // different halves of the job — `Invisible` is why it cannot be seen and
    // `Hidden` is why it is not drawn — and an item wearing one without the
    // other is a bug in either direction.
    for seed in 0..40u64 {
        let mut w = test_world(seed);
        for _ in 0..5 {
            let invisible: Vec<Entity> = w
                .query_filtered::<Entity, (With<Item>, With<Invisible>)>()
                .iter(&w)
                .collect();
            for item in invisible {
                assert!(
                    w.get::<Hidden>(item).is_some(),
                    "seed {seed}: an invisible item the renderer would still draw"
                );
            }
            descend(&mut w);
        }
    }
}
