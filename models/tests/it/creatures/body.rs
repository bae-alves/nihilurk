//! Playing as a monster: `-am <species>` hands the player a bestiary row's
//! body instead of nihil's own. The species' stats, glyph and innate
//! magic replace the player's; the starting pack does not survive the swap;
//! and a body that has no hands cannot put anything on.

use crate::common;

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

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

/// A wild dragon breathes the same spell a dragon-bodied player casts: its
/// rule set picks it from the spellset its row gives it, and fires it at
/// the player it has noticed.
#[test]
fn a_dragon_npc_breathes_the_same_spell() {
    let mut w = test_world(7, Body::Row(body_row("nihil").unwrap()));
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

fn logged(w: &World, line: &str) -> bool {
    let log = w.resource::<GameLog>();
    log.unread.iter().any(|e| e == line) || log.history.iter().any(|h| h == line)
}

#[test]
fn nobody_else_is_called_a_monster() {
    for body in [
        Body::Row(body_row("nihil").unwrap()),
        Body::Monster(MonsterDef::named("dragon")),
    ] {
        let w = test_world(7, body);
        assert!(!logged(&w, strings::you_monster()));
    }
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
fn a_launcher_species_starts_with_the_bow_on_and_a_full_stack_of_arrows() {
    let mut w = test_world(7, Body::Monster(MonsterDef::named("centaur")));
    let p = player(&mut w);
    let worn = equipment::equipped_items(&w, p);
    assert_eq!(worn.len(), 1);
    assert!(w.get::<Launcher>(worn[0]).is_some());

    let pack = w.get::<Backpack>(p).unwrap().items.clone();
    assert_eq!(pack, vec![worn[0], pack[1]], "the bow, then its arrows");
    assert_eq!(w.get::<Stack>(pack[1]).unwrap().count, STACK_LIMIT);
}

#[derive(Component)]
struct Worn;

fn put_on_the_test_body(world: &mut World, player: Entity) {
    world.entity_mut(player).insert(Worn);
}

static PACKLESS: BodyDef = BodyDef {
    name: "body of the test",
    brings_a_pack: false,
    wear: put_on_the_test_body,
};

#[test]
fn a_body_row_wears_what_it_says() {
    let mut w = test_world(7, Body::Row(&PACKLESS));
    let p = player(&mut w);
    assert!(w.get::<Worn>(p).is_some());
    assert_eq!(Body::Row(&PACKLESS).name(), "body of the test");
}
