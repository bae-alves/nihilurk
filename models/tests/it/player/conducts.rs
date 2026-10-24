//! Conducts: the 14 things a run can go without, and what a clean win pays for
//! them. One test per way of breaking one, each with a neighbour that must not.

use bevy_ecs::prelude::*;
use models::*;

fn run(body: Body) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(7)));
    w.insert_resource(RngSeed(7));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.init_resource::<UseQueue>();
    w.init_resource::<ThrowQueue>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<AttackQueue>();
    w.insert_resource(StartingBody(body));
    initialize_world(&mut w);
    w
}

fn kept(w: &mut World, c: Conduct) -> bool {
    conduct::kept_mask(w) & c.bit() != 0
}

fn player(w: &mut World) -> Entity {
    w.query_filtered::<Entity, With<Player>>().single(w)
}

fn beside(w: &mut World) -> Position {
    let p = player(w);
    let here = *w.get::<Position>(p).unwrap();
    let map = w.resource::<Map>();
    [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| (here.x as i32 + dx, here.y as i32 + dy))
        .find(|&(x, y)| x >= 0 && y >= 0 && !map.blocks(x as u16, y as u16))
        .map(|(x, y)| Position {
            x: x as u16,
            y: y as u16,
        })
        .expect("the player is walled in")
}

fn stash(w: &mut World, item: Entity) -> Entity {
    let p = player(w);
    w.entity_mut(item).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(item);
    item
}

fn use_item(w: &mut World, item: Entity) {
    let user = player(w);
    let slot = w
        .get::<Backpack>(user)
        .unwrap()
        .items
        .iter()
        .position(|&e| e == item);
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user,
        item,
        target: None,
        slot_idx: slot,
    });
    item_system(w);
}

const NOWHERE: Position = Position { x: 0, y: 0 };

fn only_broken(w: &mut World, c: Conduct) {
    let kept_now = conduct::kept_mask(w);
    let others = Conduct::ALL
        .iter()
        .filter(|&&o| o != c && o != Conduct::Lurk)
        .all(|&o| kept_now & o.bit() != 0);
    assert!(kept_now & c.bit() == 0, "{c:?} should be broken");
    assert!(others, "only {c:?} should be broken");
}

#[test]
fn invoking_a_charged_rune_breaks_runes_only() {
    let mut w = run(Body::Row(body_row("nihil").unwrap()));
    let rune = spawn_named(&mut w, "rune of recharging", NOWHERE).unwrap();
    stash(&mut w, rune);
    use_item(&mut w, rune);
    only_broken(&mut w, Conduct::Runes);
}

#[test]
fn the_angels_test_of_faith_breaks_damage() {
    let mut w = run(Body::Row(body_row("nihil").unwrap()));
    let p = player(&mut w);
    let at = beside(&mut w);
    w.get_mut::<Fighter>(p).unwrap().hp = 21;
    let spirit = spawn_monster(&mut w, MonsterDef::named("angel"), at);
    resolve_attack(&mut w, p, spirit);
    assert!(!kept(&mut w, Conduct::Damage));
    assert!(
        kept(&mut w, Conduct::Attacking),
        "a peaceful spirit is not swung at"
    );
}

fn score_of(w: &mut World) -> i64 {
    let p = player(w);
    w.get::<Score>(p).unwrap().value
}

#[test]
fn a_win_pays_ten_percent_of_the_score_for_each_conduct_kept() {
    let mut w = run(Body::Row(body_row("nihil").unwrap()));
    let p = player(&mut w);
    w.get_mut::<Score>(p).unwrap().value = 1_000;
    conduct::broke(&mut w, Conduct::Potions);

    let mask = pay_conducts(&mut w);

    assert_eq!(mask.count_ones(), 12);
    assert_eq!(score_of(&mut w), 1_000 + 1_000 * 120 / 100);
}

#[test]
fn marks_run_the_rainbow_in_order_and_the_lurk_keeps_its_own_colour() {
    use models::Color::*;
    let stripes = [Red, Green];
    let mask = Conduct::Potions.bit() | Conduct::Wands.bit() | Conduct::Lurk.bit();

    let colours: Vec<_> = conduct::marks(mask, &stripes)
        .into_iter()
        .map(|(_, c)| c)
        .collect();

    assert_eq!(colours, vec![Red, Red, conduct::LURK_COLOR]);
}
