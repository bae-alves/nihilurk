//! Playing as a monster: `-am <species>` hands the player a bestiary row's
//! body instead of nihil's own. The species' stats, glyph and innate
//! magic replace the player's; the starting pack does not survive the swap;
//! and a body that has no hands cannot put anything on.

mod common;

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64, body: Body) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<SpellQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.insert_resource(StartingBody(body));
    initialize_world(&mut w);
    w
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

#[test]
fn the_body_replaces_stats_glyph_and_innate_magic() {
    let dragon = MonsterDef::named("dragon");
    let mut w = test_world(7, Body::Monster(dragon));
    let p = player(&mut w);

    let r = w.get::<Renderable>(p).unwrap();
    assert_eq!(r.glyph, dragon.glyph);
    assert_eq!(r.color, dragon.color);

    let f = w.get::<Fighter>(p).unwrap();
    assert_eq!((f.hp, f.max_hp), (dragon.hp, dragon.hp));
    assert_eq!((f.power, f.power_bonus), (dragon.power, dragon.power_bonus));
    assert_eq!((f.armor, f.armor_bonus), (dragon.armor, dragon.armor_bonus));

    // The bestiary row's grants, on the player, as components.
    assert!(w.get::<FireImmune>(p).is_some());
    assert!(w.get::<Flies>(p).is_some());

    // Still the player, still on the player's side.
    assert!(w.get::<Player>(p).is_some());
    assert_eq!(*w.get::<Faction>(p).unwrap(), Faction::Player);
    assert_eq!(w.get::<Name>(p).unwrap().what, dragon.name);
}

#[test]
fn a_monster_starts_with_an_empty_pack() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    assert!(w.get::<Backpack>(p).unwrap().items.is_empty());
    assert_eq!(equipment::equipped_items(&w, p).len(), 0);
}

#[test]
fn breath_is_a_spell_the_body_knows_for_free() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    assert!(
        w.get::<Spellset>(p)
            .unwrap()
            .slots
            .contains(&SpellEffect::DragonBreath)
    );
    // Innate: the body pays nothing to use what it was born with.
    assert_eq!(spell_cost(&w, p, SpellEffect::DragonBreath), 0);

    // Learned from a hero coin by nihil, the same spell costs its row.
    let mut nihil = test_world(7, Body::Nihil);
    let h = player(&mut nihil);
    nihil
        .get_mut::<Spellset>(h)
        .unwrap()
        .slots
        .push(SpellEffect::DragonBreath);
    assert_eq!(
        spell_cost(&nihil, h, SpellEffect::DragonBreath),
        SpellDef::of(SpellEffect::DragonBreath).cost
    );
}

#[test]
fn a_dragon_npc_breathes_the_same_spell() {
    let mut w = test_world(7, Body::Nihil);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    w.get_mut::<Fighter>(p).unwrap().hp = 99;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 99;

    let dragon = spawn_monster(&mut w, MonsterDef::named("dragon"), here);
    let row = ABILITIES
        .iter()
        .find(|a| matches!(a.when, Moment::InsteadOfAttacking(_)))
        .expect("the breath row");
    assert!((row.action)(&mut w, dragon, Some(p)));
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 99,
        "the blast burned the player it was aimed at"
    );
}

#[test]
fn only_a_body_with_hands_can_equip() {
    // A dragon has no ItemUser on its row: claws, no straps.
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let sword = spawn_named(&mut w, "long sword", at).unwrap();
    assert!(!equipment::toggle_equipped(&mut w, p, sword));
    assert!(equipment::equipped_items(&w, p).is_empty());

    // An orc uses items, and so does an orc-bodied player.
    let mut w = test_world(7, Body::Monster(MonsterDef::named("orc")));
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let sword = spawn_named(&mut w, "long sword", at).unwrap();
    assert!(equipment::toggle_equipped(&mut w, p, sword));

    // And nihil's own hands still work.
    let mut w = test_world(7, Body::Nihil);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let sword = spawn_named(&mut w, "long sword", at).unwrap();
    assert!(equipment::toggle_equipped(&mut w, p, sword));
}

#[test]
fn an_innate_tempo_survives_the_stairs() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("wraith")));
    let p = player(&mut w);
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);

    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .unwrap();
    w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, true));

    // A floor change lifts haste the *floor* lent. It cannot lift what the
    // body was born with.
    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);
}

#[test]
fn the_body_comes_back_from_a_save() {
    let body = MonsterDef::named("wraith");
    let mut w = test_world(7, Body::Monster(body));
    let p = player(&mut w);
    // Take a bite out of it, so what comes back is this wraith and not a
    // freshly rolled one.
    w.get_mut::<Fighter>(p).unwrap().hp -= 1;
    let hp = w.get::<Fighter>(p).unwrap().hp;

    let save = common::SaveFile::new("body");
    save_game(&mut w, save.path()).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(99)));
    w2.insert_resource(RngSeed(99));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    load_game(&mut w2, save.path()).unwrap();

    let p2 = player(&mut w2);
    assert_eq!(w2.get::<Name>(p2).unwrap().what, body.name);
    assert_eq!(w2.get::<Renderable>(p2).unwrap().glyph, body.glyph);
    assert_eq!(w2.get::<Fighter>(p2).unwrap().hp, hp);
    assert_eq!(w2.get::<Speed>(p2).unwrap().kind, SpeedKind::Fast);
    assert!(w2.get::<Undead>(p2).is_some(), "innate magic came back");
    // And the costume itself, which is what the staircase and the equip gate
    // ask for — it is rebuilt from the name, not stored.
    assert!(w2.get::<MonsterBody>(p2).is_some());
}

/// The rule the save file leans on: a player named after a species *is* one
/// wearing that body, so no ordinary player name may be a species. The arg
/// parser enforces it (`nihilurk Dragon` is refused); this is the predicate
/// it enforces it with, and it has to catch every spelling a person would
/// type, not just the bestiary's own.
#[test]
fn no_player_name_can_be_mistaken_for_a_species() {
    for def in BESTIARY {
        assert!(MonsterDef::is_species_name(def.name));
        assert!(MonsterDef::is_species_name(&def.name.to_ascii_uppercase()));
        assert!(MonsterDef::is_species_name(&capitalised(def.name)));
    }
    assert!(!MonsterDef::is_species_name("nihil"));
    assert!(!MonsterDef::is_species_name("NIHIL"));
    assert!(!MonsterDef::is_species_name("bae"));
}

fn capitalised(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}
