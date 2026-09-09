//! The effect vocabulary itself: that a creature's innate magic and a piece of
//! gear's lent magic really are the same components, read by the same code.
//!
//! These tests exist to pin the decoupling down. If someone reintroduces a
//! `match ring_effect { ... }` in a subsystem, the "gear grants what a monster
//! is born with" tests below are what should start failing.

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

/// A grant list exactly as a catalog row would spell it.
const FIRE_RESISTANCE: &[Grant] = &[Grant::of::<FireImmune>()];

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// Builds exactly what a new [`RingDef`] row would put in the world — a ring
/// item carrying modifier components and a grant list — without adding one to
/// the shipped catalog. If this needs more than components to work, the design
/// has leaked.
fn custom_ring(
    w: &mut World,
    name: &str,
    grants: &'static [Grant],
    modifiers: impl Bundle,
) -> Entity {
    w.spawn((
        Name { what: name.into() },
        Item,
        Ring {
            effect: RingEffect::Adornment,
        },
        Equipped::loose(Slot::Finger),
        Grants(grants),
        modifiers,
    ))
    .id()
}

/// Put `item` on the player the way the pack screen does.
fn wear(w: &mut World, p: Entity, item: Entity) {
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    toggle_equipped(w, p, item);
}

#[test]
fn a_ring_can_grant_what_a_monster_is_born_with() {
    let mut w = test_world(1);
    let p = player(&mut w);

    // The dragon's innate immunity is a component, nothing more.
    let dragon = spawn_monster(
        &mut w,
        MonsterDef::named("dragon"),
        Position { x: 10, y: 10 },
    );
    assert!(w.get::<FireImmune>(dragon).is_some());
    assert!(w.get::<FireImmune>(p).is_none());

    // A "ring of fire resistance" is one catalog row: the same component, lent.
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());
    wear(&mut w, p, ring);

    assert!(
        w.get::<FireImmune>(p).is_some(),
        "the wearer answers the same query the dragon does"
    );
}

#[test]
fn taking_the_ring_off_takes_the_effect_with_it() {
    let mut w = test_world(2);
    let p = player(&mut w);
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());

    wear(&mut w, p, ring);
    assert!(w.get::<FireImmune>(p).is_some());

    toggle_equipped(&mut w, p, ring);
    assert!(
        w.get::<FireImmune>(p).is_none(),
        "lent magic goes back with the ring"
    );
}

#[test]
fn a_removed_ring_never_strips_innate_magic() {
    let mut w = test_world(3);
    let dragon = spawn_monster(
        &mut w,
        MonsterDef::named("dragon"),
        Position { x: 10, y: 10 },
    );
    w.entity_mut(dragon).insert(Backpack { items: Vec::new() });

    // Hand the dragon a ring of the immunity it already has, then take it away.
    let ring = custom_ring(&mut w, "ring of fire resistance", FIRE_RESISTANCE, ());
    wear(&mut w, dragon, ring);
    toggle_equipped(&mut w, dragon, ring);

    assert!(
        w.get::<FireImmune>(dragon).is_some(),
        "the dragon was born with it; no ring can take it"
    );
}

#[test]
fn a_rings_armor_bonus_folds_in_exactly_like_armour() {
    let mut w = test_world(4);
    let p = player(&mut w);

    // The player starts in +1 ring mail, so measure the ring against that base.
    let base = equipped_total::<ArmorBonus>(&w, p);

    let plus_three = custom_ring(&mut w, "ring of protection", &[], ArmorBonus(3));
    assert_eq!(equipped_total::<ArmorBonus>(&w, p), base);

    wear(&mut w, p, plus_three);
    assert_eq!(
        equipped_total::<ArmorBonus>(&w, p),
        base + 3,
        "combat and the HUD both read this one number"
    );

    // And a suit of armour lands in the very same fold. Wearing it swaps out the
    // starting ring mail, so its +1 replaces the base rather than adding to it.
    let mail = spawn_armor(&mut w, "plate mail", Position { x: 0, y: 0 });
    w.entity_mut(mail).insert(ArmorBonus(1));
    wear(&mut w, p, mail);
    assert_eq!(equipped_total::<ArmorBonus>(&w, p), 4);
    assert_eq!(
        equipped_total::<ArmorDie>(&w, p),
        9,
        "plate mail's own class"
    );
}

#[test]
fn cancellation_strips_every_effect_in_the_registry() {
    let mut w = test_world(5);
    let phantom = spawn_monster(
        &mut w,
        MonsterDef::named("phantom"),
        Position { x: 10, y: 10 },
    );
    assert!(w.get::<Undead>(phantom).is_some());

    revoke_all(&mut w, phantom);

    for grant in EFFECTS {
        assert!(
            !grant.probe(&w, phantom),
            "cancellation walks the whole registry"
        );
    }
}

#[test]
fn every_ring_in_the_catalog_has_an_appearance_and_a_row() {
    // The identification system derives its list from the catalog, so the two
    // can't drift apart. Same for the other three categories.
    let w = test_world(6);
    let appearances = w.resource::<ItemAppearances>();
    for def in RINGS {
        assert!(
            appearances.rings.contains_key(&def.effect),
            "{} has no appearance",
            def.name
        );
        assert_eq!(RingDef::of(def.effect).name, def.name);
    }
    for def in POTIONS {
        assert!(
            appearances.potions.contains_key(&def.effect),
            "{} has no appearance",
            def.name
        );
    }
    for def in SCROLLS {
        assert!(
            appearances.scrolls.contains_key(&def.effect),
            "{} has no appearance",
            def.name
        );
    }
    for def in WANDS {
        assert!(
            appearances.wands.contains_key(&def.effect),
            "{} has no appearance",
            def.name
        );
    }
}
