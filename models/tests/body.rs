//! Playing as a monster: `-am <species>` hands the player a bestiary row's
//! body instead of nihil's own. The species' stats, glyph and innate
//! magic replace the player's; the starting pack does not survive the swap;
//! and a body that has no hands cannot put anything on.

mod common;

use bevy_ecs::prelude::*;
use models::*;

fn test_world(seed: u64, body: Body) -> World {
    named_world(seed, "TESTER", body)
}

fn named_world(seed: u64, name: &str, body: Body) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<SpellQueue>();
    w.insert_resource(PlayerName { what: name.into() });
    w.insert_resource(StartingBody(body));
    initialize_world(&mut w);
    w
}

fn reloaded(w: &mut World, tag: &str) -> World {
    let save = common::SaveFile::new(tag);
    save_game(w, save.path()).unwrap();
    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(99)));
    w2.insert_resource(RngSeed(99));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();
    w2
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

    assert!(w.get::<FireImmune>(p).is_some());
    assert!(w.get::<Flies>(p).is_some());

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
fn breath_is_a_spell_the_body_pays_for_like_any_other() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    assert!(
        w.get::<Spellset>(p)
            .unwrap()
            .slots
            .contains(&SpellEffect::DragonBreath)
    );
    assert_eq!(
        spell_cost(&w, p, SpellEffect::DragonBreath),
        SpellDef::of(SpellEffect::DragonBreath).cost
    );

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

/// A wild dragon breathes the same spell a dragon-bodied player casts: its
/// rule set picks it from the spellset its row gives it, and fires it at
/// the player it has noticed.
#[test]
fn a_dragon_npc_breathes_the_same_spell() {
    let mut w = test_world(7, Body::Nihil);
    w.init_resource::<AttackQueue>();
    let strays: Vec<Entity> = w
        .query_filtered::<Entity, (With<Mob>, Without<Player>)>()
        .iter(&w)
        .collect();
    for e in strays {
        w.despawn(e);
    }
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    w.get_mut::<Fighter>(p).unwrap().hp = 99;
    w.get_mut::<Fighter>(p).unwrap().max_hp = 99;

    let there = Position {
        x: here.x + 3,
        y: here.y,
    };
    assert!(w.resource::<Map>().walkable(there.x, there.y, false));
    spawn_monster(&mut w, MonsterDef::named("dragon"), there);
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(&mut w);
    ai(&mut w);
    assert!(
        w.get::<Fighter>(p).unwrap().hp < 99,
        "the blast burned the player it was aimed at"
    );
}

#[test]
fn only_a_body_with_hands_can_equip() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let sword = spawn_named(&mut w, "long sword", at).unwrap();
    assert!(!equipment::toggle_equipped(&mut w, p, sword));
    assert!(equipment::equipped_items(&w, p).is_empty());

    let mut w = test_world(7, Body::Monster(MonsterDef::named("orc")));
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let sword = spawn_named(&mut w, "long sword", at).unwrap();
    assert!(equipment::toggle_equipped(&mut w, p, sword));

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

    assert_eq!(w.get::<Speed>(p).unwrap().kind, SpeedKind::Fast);
}

#[test]
fn the_body_comes_back_from_a_save() {
    let body = MonsterDef::named("wraith");
    let mut w = test_world(7, Body::Monster(body));
    let p = player(&mut w);
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
    assert!(w2.get::<MonsterBody>(p2).is_some());
}

#[test]
fn a_player_named_after_a_species_is_still_nihil_after_a_save() {
    let mut w = named_world(7, "dragon", Body::Nihil);
    let p = player(&mut w);
    let hp = w.get::<Fighter>(p).unwrap().max_hp;

    let mut w2 = reloaded(&mut w, "named-dragon");

    let p2 = player(&mut w2);
    assert!(w2.get::<MonsterBody>(p2).is_none(), "no costume was saved");
    assert!(w2.get::<FireImmune>(p2).is_none(), "no dragon magic");
    assert_eq!(w2.get::<Fighter>(p2).unwrap().max_hp, hp);
    assert!(
        w2.get_resource::<StartingBody>()
            .is_none_or(|b| !matches!(b.0, Body::Monster(_))),
        "the run did not start as a monster"
    );
    assert_eq!(w2.get::<Name>(p2).unwrap().what, "dragon");
}

fn logged(w: &World, line: &str) -> bool {
    let log = w.resource::<GameLog>();
    log.unread.iter().any(|e| e == line) || log.history.iter().any(|h| h == line)
}

#[test]
fn an_apis_is_called_a_monster_on_the_first_turn() {
    let w = test_world(7, Body::Monster(MonsterDef::named("apis")));
    assert!(logged(&w, strings::you_monster()));
}

#[test]
fn nobody_else_is_called_a_monster() {
    for body in [Body::Nihil, Body::Monster(MonsterDef::named("dragon"))] {
        let w = test_world(7, body);
        assert!(!logged(&w, strings::you_monster()));
    }
}

/// The `Map` alone for `(seed, depth)`, as an apis player would get it.
fn bee_map_at(seed: u64, depth: u8) -> Map {
    let mut w = World::new();
    w.insert_resource(StartingBody(Body::Monster(MonsterDef::named("apis"))));
    regenerate_map(&mut w, seed, depth);
    w.remove_resource::<Map>().unwrap()
}

#[test]
fn an_apis_run_makes_every_eligible_floor_a_bee_world() {
    for seed in 0..40 {
        for depth in constants::map::SPECIAL_LEVEL_MIN_DEPTH..FINAL_DEPTH {
            let m = bee_map_at(seed, depth);
            assert_eq!(
                m.level,
                Some(SpecialLevel::BeeWorld),
                "seed {seed} depth {depth}"
            );
        }
    }
}

#[test]
fn an_apis_run_only_has_hives_for_special_rooms_about_a_tenth_of_the_time() {
    let (mut rooms, mut hives) = (0usize, 0usize);
    for seed in 0..300 {
        let m = bee_map_at(seed, 3);
        assert_eq!(m.level, None);
        let kinds: Vec<_> = m.special.iter().flatten().collect();
        assert!(kinds.iter().all(|k| **k == SpecialRoom::TreasureHive));
        hives += (!kinds.is_empty()) as usize;
        rooms += 1;
    }
    assert!(
        hives * 10 > rooms * 2 && hives * 10 < rooms * 6,
        "{hives} of {rooms} floors had a hive"
    );
}

#[test]
fn a_saved_apis_run_stays_a_bee_run() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("apis")));
    let path = std::env::temp_dir().join("nihilurk-bee-save-test.sav");
    let path = path.to_str().unwrap();
    save_game(&mut w, path).unwrap();
    let mut loaded = World::new();
    load_game(&mut loaded, path).unwrap();
    std::fs::remove_file(path).ok();
    let depth = constants::map::SPECIAL_LEVEL_MIN_DEPTH;
    regenerate_map(&mut loaded, 7, depth);
    assert_eq!(loaded.resource::<Map>().level, Some(SpecialLevel::BeeWorld));
}

/// Every line the run's log has said.
fn said(w: &World) -> Vec<String> {
    w.resource::<GameLog>().history.to_vec()
}

/// Wearing a creature tells you what you were born with: one "you feel" line
/// per grant, the random boons a spirit rolls included.
#[test]
fn a_worn_body_feels_every_grant_it_was_born_with() {
    for def in BESTIARY {
        let mut w = test_world(3, Body::Monster(def));
        let p = player(&mut w);
        let log = said(&w);
        let born: Vec<&str> = w
            .get::<Effects>(p)
            .map(|e| {
                e.0.iter()
                    .filter(|h| !matches!(h.lifetime, Lifetime::WhileEquipped(_)))
                    .map(|h| h.id)
                    .collect()
            })
            .unwrap_or_default();
        for id in &born {
            let feel = Effect::by_id(id)
                .and_then(|e| e.feel)
                .unwrap_or_else(|| panic!("{}: no feel line for {id}", def.name));
            assert!(
                log.iter().any(|l| l == feel),
                "{}: never said {feel:?}",
                def.name
            );
        }
    }
}

#[test]
fn a_worn_spirit_is_born_with_its_boons() {
    let mut w = test_world(3, Body::Monster(MonsterDef::named("sylphid")));
    let p = player(&mut w);
    let held = w.get::<Effects>(p).unwrap().0.len();
    let fixed = MonsterDef::named("sylphid").grants.len();
    assert_eq!(held, fixed + 2);
}

#[test]
fn nihil_feels_nothing_at_the_start() {
    let w = test_world(3, Body::Nihil);
    let feels: Vec<&str> = EFFECTS.iter().filter_map(|e| e.feel).collect();
    assert!(!said(&w).iter().any(|l| feels.contains(&l.as_str())));
}

#[test]
fn a_species_with_gear_chances_starts_wearing_all_of_it() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("hobgoblin")));
    let p = player(&mut w);
    assert_eq!(equipment::equipped_items(&w, p).len(), 3);
    assert!(w.get::<Backpack>(p).unwrap().items.is_empty());
}

#[test]
fn a_launcher_species_starts_with_the_bow_on_and_a_full_stack_of_arrows() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("centaur")));
    let p = player(&mut w);
    let worn = equipment::equipped_items(&w, p);
    assert_eq!(worn.len(), 1);
    assert!(w.get::<Launcher>(worn[0]).is_some());

    let pack = w.get::<Backpack>(p).unwrap().items.clone();
    assert_eq!(pack.len(), 1);
    assert_eq!(w.get::<Stack>(pack[0]).unwrap().count, STACK_LIMIT);
}

#[test]
fn a_species_without_gear_chances_starts_bare() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("dragon")));
    let p = player(&mut w);
    assert!(equipment::equipped_items(&w, p).is_empty());
    assert!(w.get::<Backpack>(p).unwrap().items.is_empty());
}
