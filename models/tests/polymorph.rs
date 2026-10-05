//! Polymorph on the player: a floor-long loan of a species' powers, held in
//! the effect ledger, and nothing else about them changes.
//!
//! The species is rolled by the wand, the potion and the ring; every test here
//! picks one with `polymorph_player_into` so the claim is about the mechanic,
//! not about a seed.

#[path = "common/mod.rs"]
mod common;

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
fn polymorph_lends_the_powers_and_changes_nothing_else() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let name = w.get::<Name>(p).unwrap().what.clone();
    let glyph = w.get::<Renderable>(p).unwrap().glyph;
    let hp = w.get::<Fighter>(p).unwrap().max_hp;

    polymorph_player_into(&mut w, p, species("dragon"));

    assert!(w.get::<Polymorphed>(p).is_some(), "the badge is on");
    assert!(
        w.get::<FireImmune>(p).is_some(),
        "the dragon's fire is lent"
    );
    assert_eq!(w.get::<Name>(p).unwrap().what, name);
    assert_eq!(w.get::<Renderable>(p).unwrap().glyph, glyph);
    assert_eq!(w.get::<Fighter>(p).unwrap().max_hp, hp);
    assert!(w.get::<MonsterBody>(p).is_none(), "no body was swapped in");
}

#[test]
fn a_polymorph_lasts_the_floor_and_no_longer() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));

    descend(&mut w, p);

    assert!(w.get::<Polymorphed>(p).is_none());
    assert!(w.get::<FireImmune>(p).is_none());
    assert!(
        effects_of(&w, p)
            .iter()
            .all(|h| h.lifetime != Lifetime::Floor),
        "nothing on loan is left in the ledger"
    );
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l.contains("no longer polymorphed")),
        "and the player is told"
    );
}

#[test]
fn cancellation_ends_a_polymorph() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));

    revoke_all(&mut w, p);

    assert!(w.get::<Polymorphed>(p).is_none());
    assert!(w.get::<FireImmune>(p).is_none());
}

#[test]
fn a_species_that_is_what_a_creature_is_lends_no_identity() {
    let mut w = test_world(3);
    let p = player(&mut w);

    polymorph_player_into(&mut w, p, species("dog"));

    assert!(
        w.get::<AlwaysTamed>(p).is_none(),
        "a dog's tameness is its identity, and the player is not on the dog's side"
    );
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
fn the_player_keeps_no_spells_from_the_species() {
    let mut w = test_world(3);
    let p = player(&mut w);
    let before = w.get::<Spellset>(p).unwrap().slots.clone();

    polymorph_player_into(&mut w, p, species("dragon"));

    assert_eq!(
        w.get::<Spellset>(p).unwrap().slots,
        before,
        "the breath is not in the spell bar"
    );
}

#[test]
fn a_polymorph_survives_a_save() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));

    let save = common::SaveFile::new("polymorph");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2)));
    w2.insert_resource(RngSeed(2));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let p2 = player(&mut w2);
    assert!(w2.get::<Polymorphed>(p2).is_some());
    assert!(w2.get::<FireImmune>(p2).is_some());
    assert!(
        w2.get::<MonsterBody>(p2).is_none(),
        "a saved polymorph is not a permanent costume"
    );
}

#[test]
fn every_species_lends_exactly_its_grants_and_they_all_come_back() {
    for def in BESTIARY.iter().filter(|m| m.spirit_kind.is_none()) {
        let mut w = test_world(3);
        let p = player(&mut w);

        polymorph_player_into(&mut w, p, def);

        assert!(w.get::<Polymorphed>(p).is_some(), "{}: no badge", def.name);
        for grant in def.grants.iter().filter(|g| !g.is_identity()) {
            assert!(grant.probe(&w, p), "{}: a grant was not lent", def.name);
        }
        for grant in def.grants.iter().filter(|g| g.is_identity()) {
            assert!(!grant.probe(&w, p), "{}: lent what it *is*", def.name);
        }

        clear_floor_grants(&mut w, p);

        for grant in def.grants {
            assert!(!grant.probe(&w, p), "{}: a grant stayed behind", def.name);
        }
        assert!(
            w.get::<Polymorphed>(p).is_none(),
            "{}: badge stayed",
            def.name
        );
    }
}

#[test]
fn a_second_polymorph_makes_a_chimeric_form_and_the_first_does_not() {
    let mut w = test_world(3);
    let p = player(&mut w);

    polymorph_player_into(&mut w, p, species("dragon"));
    assert!(
        chimeric_form(&w, p).is_none(),
        "one polymorph is just a loan"
    );

    polymorph_player_into(&mut w, p, species("orc"));
    let form = chimeric_form(&w, p).expect("the second one stacks into a form");
    assert!(['C', 'T', 'E'].contains(&form.glyph));
    assert!(!form.name.is_empty());

    polymorph_player_into(&mut w, p, species("troll"));
    let held = FORMS.iter().filter(|f| f.grant.probe(&w, p)).count();
    assert_eq!(
        held, 1,
        "a third re-rolls the form, it does not collect them"
    );
}

#[test]
fn a_chimeric_form_ends_with_the_floor() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));
    polymorph_player_into(&mut w, p, species("orc"));
    assert!(chimeric_form(&w, p).is_some());

    descend(&mut w, p);

    assert!(chimeric_form(&w, p).is_none());
    assert!(w.get::<Polymorphed>(p).is_none());
}

#[test]
fn cancellation_takes_a_chimeric_form_too() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));
    polymorph_player_into(&mut w, p, species("orc"));

    revoke_all(&mut w, p);

    assert!(chimeric_form(&w, p).is_none());
}

#[test]
fn a_chimeric_form_survives_a_save() {
    let mut w = test_world(3);
    let p = player(&mut w);
    polymorph_player_into(&mut w, p, species("dragon"));
    polymorph_player_into(&mut w, p, species("orc"));
    let glyph = chimeric_form(&w, p).unwrap().glyph;

    let save = common::SaveFile::new("chimera");
    save_game(&mut w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2)));
    w2.insert_resource(RngSeed(2));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    let p2 = player(&mut w2);
    assert_eq!(chimeric_form(&w2, p2).map(|f| f.glyph), Some(glyph));
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
fn a_chimeric_monster_is_named_for_its_form_in_the_ward_and_immunity_lines() {
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
    lend(&mut w, mob, Grant::of::<MagicWard>(), Lifetime::Floor);

    apply_hit(
        &mut w,
        mob,
        Hit {
            amount: 3,
            element: None,
            magical: true,
        },
        None,
    );

    let log = w.resource::<GameLog>().history.join("\n");
    assert!(
        log.contains("chimera"),
        "the ward line names the form: {log}"
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
    // A dog: its grants make whatever it is polymorphed into still the
    // player's to recruit.
    spawn_monster(&mut w, species("dog"), spot);
    let wand = spawn_wand(&mut w, WandEffect::Polymorph, here);
    w.entity_mut(wand).remove::<Position>();
    let slot = w.get::<Backpack>(p).unwrap().items.len();
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
        .expect("the dog was polymorphed, not killed");
    assert!(w.get::<Polymorphed>(changed).is_some());
    charm(&mut w, changed);
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
