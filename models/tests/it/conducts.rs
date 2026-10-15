//! Conducts: the 14 things a run can go without, and what a clean win pays for
//! them. One test per way of breaking one, each with a neighbour that must not.

use crate::common;
use crate::common::monster;

use bevy_ecs::prelude::*;
use crossterm::style::Color;
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

#[test]
fn a_fresh_nihil_run_keeps_every_conduct_but_lurk() {
    let mut w = run(Body::Nihil);
    assert_eq!(conduct::kept_mask(&mut w).count_ones(), 13);
    assert!(!kept(&mut w, Conduct::Lurk));
}

#[test]
fn a_fresh_lurk_run_keeps_all_fourteen() {
    let mut w = run(Body::Lurk);
    assert_eq!(conduct::kept_mask(&mut w).count_ones(), 14);
    assert!(kept(&mut w, Conduct::Lurk));
}

#[test]
fn breaking_one_breaks_only_that_one() {
    let mut w = run(Body::Nihil);
    conduct::broke(&mut w, Conduct::Potions);
    assert!(!kept(&mut w, Conduct::Potions));
    assert!(kept(&mut w, Conduct::Scrolls));
    assert_eq!(conduct::kept_mask(&mut w).count_ones(), 12);
}

#[test]
fn breaking_a_conduct_twice_costs_nothing_more() {
    let mut w = run(Body::Nihil);
    conduct::broke(&mut w, Conduct::Coins);
    conduct::broke(&mut w, Conduct::Coins);
    assert_eq!(conduct::kept_mask(&mut w).count_ones(), 12);
}

#[test]
fn a_bare_world_has_nothing_to_break() {
    let mut w = World::new();
    conduct::broke(&mut w, Conduct::Coins);
}

#[test]
fn each_conduct_kept_is_ten_percent() {
    assert_eq!(conduct::bonus_percent(0), 0);
    assert_eq!(conduct::bonus_percent(0b111), 30);
    assert_eq!(conduct::bonus_percent(0x3fff), 140);
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
    if let Some(i) = slot {
        w.get_mut::<Backpack>(user).unwrap().items.remove(i);
    }
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
fn drinking_a_potion_breaks_potions_only() {
    let mut w = run(Body::Nihil);
    let potion = spawn_potion(&mut w, PotionEffect::Confusion, NOWHERE);
    stash(&mut w, potion);
    use_item(&mut w, potion);
    only_broken(&mut w, Conduct::Potions);
}

#[test]
fn reading_a_scroll_breaks_scrolls_only() {
    let mut w = run(Body::Nihil);
    let scroll = spawn_scroll(&mut w, ScrollEffect::MagicMapping, NOWHERE);
    stash(&mut w, scroll);
    use_item(&mut w, scroll);
    only_broken(&mut w, Conduct::Scrolls);
}

#[test]
fn zapping_a_wand_breaks_wands_only() {
    let mut w = run(Body::Nihil);
    let wand = spawn_wand(&mut w, WandEffect::Light, NOWHERE);
    stash(&mut w, wand);
    use_item(&mut w, wand);
    only_broken(&mut w, Conduct::Wands);
}

#[test]
fn invoking_a_charged_rune_breaks_runes_only() {
    let mut w = run(Body::Nihil);
    let rune = spawn_rune(&mut w, RuneEffect::Recharging, NOWHERE);
    stash(&mut w, rune);
    use_item(&mut w, rune);
    only_broken(&mut w, Conduct::Runes);
}

#[test]
fn an_inert_rune_breaks_nothing() {
    let mut w = run(Body::Nihil);
    let rune = spawn_rune(&mut w, RuneEffect::Recharging, NOWHERE);
    w.get_mut::<Rune>(rune).unwrap().charged = false;
    stash(&mut w, rune);
    use_item(&mut w, rune);
    assert_eq!(conduct::kept_mask(&mut w).count_ones(), 13);
}

#[test]
fn casting_a_spell_breaks_spells_only() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let here = *w.get::<Position>(p).unwrap();
    w.get_mut::<Magic>(p).unwrap().points = 99;
    w.resource_mut::<SpellQueue>().spells.push(WantsToCast {
        user: p,
        effect: SpellEffect::Setup,
        target: here,
    });
    spell_system(&mut w);
    only_broken(&mut w, Conduct::Spells);
}

#[test]
fn taking_a_coin_breaks_coins_only() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let at = *w.get::<Position>(p).unwrap();
    let coin = spawn_named(&mut w, "gold coin", at).expect("a coin");
    pick_up(&mut w, p, coin).expect("taken");
    only_broken(&mut w, Conduct::Coins);
}

#[test]
fn throwing_something_breaks_ranged_only() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let at = beside(&mut w);
    let potion = spawn_potion(&mut w, PotionEffect::Confusion, NOWHERE);
    stash(&mut w, potion);
    w.get_mut::<Backpack>(p)
        .unwrap()
        .items
        .retain(|&e| e != potion);
    w.resource_mut::<ThrowQueue>().throws.push(WantsToThrow {
        thrower: p,
        item: potion,
        target: at,
        slot_idx: None,
    });
    throw_system(&mut w);
    only_broken(&mut w, Conduct::Ranged);
}

#[test]
fn a_melee_swing_breaks_attacking_only() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let at = beside(&mut w);
    let foe = monster::plain_monster(&mut w, "dummy", at);
    melee_attack(&mut w, p, foe);
    assert_eq!(conduct::kept_mask(&mut w) & Conduct::Attacking.bit(), 0);
}

#[test]
fn being_swung_at_does_not_break_attacking() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let at = beside(&mut w);
    let foe = monster::plain_monster(&mut w, "dummy", at);
    melee_attack(&mut w, foe, p);
    assert!(kept(&mut w, Conduct::Attacking));
}

#[test]
fn charming_breaks_charming_and_recruiting_breaks_companion() {
    let mut w = run(Body::Nihil);
    let at = beside(&mut w);
    let a = monster::plain_monster(&mut w, "a", at);
    charm(&mut w, a);
    only_broken(&mut w, Conduct::Charming);
    let b = monster::plain_monster(&mut w, "b", at);
    recruit(&mut w, b);
    assert!(!kept(&mut w, Conduct::Companion));
}

#[test]
fn putting_gear_on_breaks_gear_but_the_starting_kit_and_taking_it_off_do_not() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    assert!(kept(&mut w, Conduct::Gear));
    let mail = equipped_in(&w, p, Slot::Body).expect("nihil starts in armour");
    toggle_equipped(&mut w, p, mail);
    assert!(kept(&mut w, Conduct::Gear));
    toggle_equipped(&mut w, p, mail);
    assert!(!kept(&mut w, Conduct::Gear));
}

#[test]
fn armour_taking_the_killing_blow_breaks_armor() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let mail = equipped_in(&w, p, Slot::Body).unwrap();
    w.entity_mut(mail).insert(ArmorBonus(2));
    w.get_mut::<Fighter>(p).unwrap().hp = 3;
    apply_hit(&mut w, p, Hit::magic(50), None);
    assert!(!kept(&mut w, Conduct::Armor));
}

#[test]
fn a_blow_that_does_not_kill_leaves_armor_kept() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    apply_hit(&mut w, p, Hit::magic(1), None);
    assert!(kept(&mut w, Conduct::Armor));
}

#[test]
fn losing_hp_breaks_damage_and_hurting_a_monster_does_not() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    let at = beside(&mut w);
    let foe = monster::plain_monster(&mut w, "dummy", at);
    apply_hit(&mut w, foe, Hit::physical(5), None);
    assert!(kept(&mut w, Conduct::Damage));
    apply_hit(&mut w, p, Hit::physical(1), None);
    assert!(!kept(&mut w, Conduct::Damage));
}

#[test]
fn the_angels_test_of_faith_breaks_damage() {
    let mut w = run(Body::Nihil);
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
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    w.get_mut::<Score>(p).unwrap().value = 1_000;
    conduct::broke(&mut w, Conduct::Potions);

    let mask = pay_conducts(&mut w);

    assert_eq!(mask.count_ones(), 12);
    assert_eq!(score_of(&mut w), 1_000 + 1_000 * 120 / 100);
}

#[test]
fn a_clean_lurk_win_pays_one_hundred_forty_percent() {
    let mut w = run(Body::Lurk);
    let p = player(&mut w);
    w.get_mut::<Score>(p).unwrap().value = 500;

    pay_conducts(&mut w);

    assert_eq!(score_of(&mut w), 500 + 700);
}

#[test]
fn a_run_that_kept_nothing_is_paid_nothing() {
    let mut w = run(Body::Nihil);
    let p = player(&mut w);
    w.get_mut::<Score>(p).unwrap().value = 1_000;
    for c in Conduct::ALL {
        conduct::broke(&mut w, c);
    }

    pay_conducts(&mut w);

    assert_eq!(score_of(&mut w), 1_000);
}

#[test]
fn broken_conducts_survive_a_save() {
    let mut w = run(Body::Nihil);
    conduct::broke(&mut w, Conduct::Potions);
    conduct::broke(&mut w, Conduct::Damage);
    let save = common::SaveFile::new("conducts");
    save_game(&mut w, save.path()).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(7)));
    w2.insert_resource(RngSeed(7));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, save.path()).unwrap();

    assert_eq!(conduct::kept_mask(&mut w2), conduct::kept_mask(&mut w));
    assert!(!kept(&mut w2, Conduct::Potions));
    assert!(kept(&mut w2, Conduct::Scrolls));
}

#[test]
fn marks_run_the_rainbow_in_order_and_the_lurk_keeps_its_own_colour() {
    use crossterm::style::Color::*;
    let stripes = [Red, Green];
    let mask = Conduct::Potions.bit() | Conduct::Wands.bit() | Conduct::Lurk.bit();

    let colours: Vec<_> = conduct::marks(mask, &stripes)
        .into_iter()
        .map(|(_, c)| c)
        .collect();

    assert_eq!(colours, vec![Red, Red, conduct::LURK_COLOR]);
}

#[test]
fn a_broken_conduct_has_no_mark() {
    let mask = Conduct::Potions.bit();
    assert_eq!(conduct::marks(mask, &[Color::Red]).len(), 1);
    assert!(conduct::marks(0, &[Color::Red]).is_empty());
}

#[test]
fn the_lurks_mark_is_the_lurks_own_colour() {
    let mut w = run(Body::Lurk);
    let p = player(&mut w);
    assert_eq!(w.get::<Renderable>(p).unwrap().color, conduct::LURK_COLOR);
}
