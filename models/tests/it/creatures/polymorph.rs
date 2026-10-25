//! Polymorph on the player: a floor-long loan of a species' powers, held in
//! the effect ledger, and nothing else about them changes.
//!
//! The species is rolled by the wand, the potion and the ring; every test here
//! picks one with `polymorph_player_into` so the claim is about the mechanic,
//! not about a seed.

use crate::common::monster;

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
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn species(name: &str) -> &'static MonsterDef {
    MonsterDef::lookup(name).expect("a bestiary row")
}

fn descend(w: &mut World, p: Entity) {
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(w, true));
}

#[test]
fn a_handless_species_drops_the_gear_and_cannot_wield_it() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let worn = equipped_items(&w, p);
    assert!(!worn.is_empty(), "nihil starts dressed");

    polymorph_player_into(&mut w, p, species("dragon"));

    assert!(
        equipped_items(&w, p).is_empty(),
        "claws cannot hold a mace, so it is back in the pack"
    );
    for item in &worn {
        assert!(
            !toggle_equipped(&mut w, p, *item),
            "and the pack's gear stays off while the shape lasts"
        );
    }
}

#[test]
fn a_species_with_hands_keeps_the_gear() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let worn = equipped_items(&w, p);

    polymorph_player_into(&mut w, p, species("orc"));

    assert_eq!(equipped_items(&w, p), worn);
}

#[test]
fn polymorphitis_never_shocks_its_bearer() {
    let mut w = test_world(5);
    let p = player(&mut w);
    let hp = w.get::<Fighter>(p).unwrap().hp;
    assert!(hp > 1);
    lend(&mut w, p, Grant::of::<Polymorphitis>(), Lifetime::Permanent);
    polymorph_player_into(&mut w, p, species("dragon"));

    let mut formed = false;
    for _ in 0..8000 {
        ability_system(&mut w);
        assert_eq!(
            w.get::<Fighter>(p).unwrap().hp,
            hp,
            "the ring rolled a system shock"
        );
        formed |= chimeric_form(&w, p).is_some();
    }
    assert!(
        formed,
        "a polymorphed bearer eventually settles into a form"
    );
}

#[test]
fn a_chimeric_monster_is_named_for_its_form_in_the_protection_line() {
    let mut w = test_world(3);
    let (_here, spot) = {
        let p = player(&mut w);
        let here = *w.get::<Position>(p).unwrap();
        (
            here,
            Position {
                x: here.x + 1,
                y: here.y,
            },
        )
    };
    let mob = w
        .spawn((
            Name { what: "orc".into() },
            Mob {
                movement_type: MovementType::Static,
            },
            spot,
            Fighter {
                hp: 50,
                max_hp: 50,
                armor: 0,
                power: 1,
                max_power: 1,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Faction::Monster,
            Speed::new(SpeedKind::Normal),
        ))
        .id();
    lend(&mut w, mob, Grant::of::<Chimera>(), Lifetime::Floor);
    w.entity_mut(mob).insert(Protected);

    apply_hit(
        &mut w,
        mob,
        Hit {
            amount: 3,
            element: None,
        },
        None,
    );

    let log = w.resource::<GameLog>().history.join("\n");
    assert!(
        log.contains("chimera"),
        "the protection line names the form: {log}"
    );
    assert!(!log.contains("orc"), "and never the species: {log}");
}

#[test]
fn a_polymorphed_ally_stays_polymorphed_down_the_stairs() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    let spot = Position {
        x: here.x + 1,
        y: here.y,
    };
    monster::plain_monster(&mut w, "pup", spot);
    let wand = spawn_named(&mut w, "wand of polymorph", here).unwrap();
    w.entity_mut(wand).remove::<Position>();
    let slot = w.get::<Backpack>(p).unwrap().items.len();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(spot),
        slot_idx: Some(slot),
    });
    item_system(&mut w);
    let mut q = w.query_filtered::<(Entity, &Position), With<Mob>>();
    let changed = q
        .iter(&w)
        .find(|(_, pos)| **pos == spot)
        .map(|(e, _)| e)
        .expect("the pup was polymorphed, not killed");
    assert!(w.get::<Polymorphed>(changed).is_some());
    recruit(&mut w, changed);
    assert_eq!(all_helpers(&mut w), vec![changed]);

    descend(&mut w, p);

    let helpers = all_helpers(&mut w);
    assert_eq!(helpers.len(), 1, "the helper followed");
    assert!(
        w.get::<Polymorphed>(helpers[0]).is_some(),
        "and it is still polymorphed"
    );
    assert!(
        w.get::<Polymorphed>(p).is_none(),
        "while the player's own loan ended"
    );
}

#[test]
fn the_query_term_and_the_world_lookup_agree_on_every_form() {
    let mut w = test_world(3);
    for form in FORMS {
        let e = w.spawn_empty().id();
        lend(&mut w, e, form.grant, Lifetime::Floor);
        let mut q = w.query::<(Entity, FormMarks)>();
        let (_, marks) = q.iter(&w).find(|(id, _)| *id == e).unwrap();
        let via_query = form_of_marks(marks).map(|f| f.glyph);
        assert_eq!(via_query, chimeric_form(&w, e).map(|f| f.glyph));
        assert_eq!(via_query, Some(form.glyph));
    }
}
