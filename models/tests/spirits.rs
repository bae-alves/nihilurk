//! `Faction::Spirits`: peaceful until crossed, either by an `Alignment` pole
//! or a direct hit. See `models::spirits`.

#[path = "common/monster.rs"]
#[allow(dead_code)] // only the handless fixture is wanted here
mod monster;

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use models::*;

fn spirits_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.init_resource::<GameLog>();
    w.init_resource::<SpiritsHostile>();
    w.init_resource::<OfferMenu>();
    w.init_resource::<BarterMenu>();
    w.init_resource::<Ending>();
    w
}

fn spawn_player(w: &mut World, power: i32) -> Entity {
    w.spawn((
        Player,
        Name { what: "you".into() },
        Fighter {
            hp: 20,
            max_hp: 20,
            armor: 0,
            power,
            max_power: power,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Alignment::default(),
        Position { x: 1, y: 1 },
        Backpack { items: Vec::new() },
        Spellset::default(),
    ))
    .id()
}

fn spawn_spirit(w: &mut World, hp: i32) -> Entity {
    w.spawn((
        Name {
            what: "spirit".into(),
        },
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Renderable {
            glyph: '&',
            color: Color::White,
        },
        Faction::Spirits,
        Blood,
    ))
    .id()
}

fn logged(w: &World, needle: &str) -> bool {
    w.resource::<GameLog>()
        .unread
        .iter()
        .any(|l| l.text.contains(needle))
}

/// Melee is intercepted before any damage roll — a peaceful spirit is never
/// actually *fought* by a sword. See `combat::resolve_attack`'s spirit
/// branch.
#[test]
fn melee_on_a_peaceful_spirit_never_deals_damage_or_flips_the_flag() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_spirit(&mut w, 100);
    resolve_attack(&mut w, player, spirit);
    assert_eq!(w.get::<Fighter>(spirit).unwrap().hp, 100);
    assert!(!w.resource::<SpiritsHostile>().0);
    assert!(logged(&w, "takes no notice"));
}

fn alignment(w: &World, player: Entity) -> i8 {
    w.get::<Alignment>(player).unwrap().0
}

#[test]
fn talking_to_a_spirit_alone_leaves_alignment_be() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_spirit(&mut w, 100);
    w.entity_mut(spirit).insert(SpiritKind::Cacodaemon);
    resolve_attack(&mut w, player, spirit);
    assert_eq!(alignment(&w, player), 0);
}

#[test]
fn a_cancelled_offer_leaves_alignment_be() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    talk_to(&mut w, player, "sylphid");
    assert!(w.resource::<OfferMenu>().open);
    assert_eq!(alignment(&w, player), 0);
}

#[test]
fn a_spirit_poofing_pulls_alignment_by_its_kind() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    talk_to(&mut w, player, "sylphid");
    let choice = offered(&w)[0];
    spirits::confirm_offer(&mut w, player, choice);
    assert_eq!(alignment(&w, player), 1);
    talk_to(&mut w, player, "red demon");
    let choice = offered(&w)[0];
    spirits::confirm_offer(&mut w, player, choice);
    assert_eq!(alignment(&w, player), 0);
}

#[test]
fn a_finished_barter_pulls_alignment() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let item = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    talk_to(&mut w, player, "yellow demon");
    spirits::confirm_barter(&mut w, player);
    assert_eq!(alignment(&w, player), -1);
}

#[test]
fn the_angel_pulls_alignment_as_they_poof() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    talk_to(&mut w, player, "angel");
    assert_eq!(alignment(&w, player), 1);
}

fn poof_event(world: &mut World, _player: Entity, spirit: Entity) {
    world.resource_mut::<GameLog>().add("It poofs!".to_string());
    world.despawn(spirit);
}

#[test]
fn a_spirit_events_own_fn_runs_instead_of_the_generic_line() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_spirit(&mut w, 100);
    w.entity_mut(spirit).insert(SpiritEvent(poof_event));
    resolve_attack(&mut w, player, spirit);
    assert!(logged(&w, "It poofs!"));
    assert!(!logged(&w, "takes no notice"));
    assert!(w.get_entity(spirit).is_none());
}

/// Spirits are fickle: a monster drawing blood angers them as surely as the
/// player does.
#[test]
fn a_monster_hitting_a_spirit_angers_them() {
    let mut w = spirits_world(0);
    let attacker = monster::plain_monster(&mut w, "orc", Position { x: 1, y: 1 });
    w.get_mut::<Fighter>(attacker).unwrap().power = 6;
    let spirit = spawn_spirit(&mut w, 100);
    for _ in 0..50 {
        resolve_attack(&mut w, attacker, spirit);
        if !w.get::<Fighter>(spirit).is_some_and(|f| f.hp == 100) {
            break;
        }
    }
    assert!(w.get::<Fighter>(spirit).unwrap().hp < 100, "never landed");
    assert!(w.resource::<SpiritsHostile>().0);
}

/// A blast, a bolt, a trap or a trick shot: every source with no swing
/// behind it hurts through `apply_hit`.
#[test]
fn any_wound_on_a_spirit_angers_them() {
    let mut w = spirits_world(0);
    let spirit = spawn_spirit(&mut w, 100);
    apply_hit(&mut w, spirit, Hit::physical(3), None);
    assert!(w.resource::<SpiritsHostile>().0);
}

#[test]
fn a_hit_that_draws_no_blood_angers_nobody() {
    let mut w = spirits_world(0);
    let spirit = spawn_spirit(&mut w, 100);
    apply_hit(&mut w, spirit, Hit::physical(0), None);
    assert!(!w.resource::<SpiritsHostile>().0);
}

#[test]
fn alignment_shift_clamps_and_flips_at_the_pole() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);

    for _ in 0..3 {
        spirits::shift_alignment(&mut w, player, -1);
    }
    assert_eq!(w.get::<Alignment>(player).unwrap().0, -3);
    assert!(w.resource::<SpiritsHostile>().0);
    assert!(logged(&w, "challenge the balance"));

    // Already at the pole: one more nudge does not go past it or panic.
    spirits::shift_alignment(&mut w, player, -1);
    assert_eq!(w.get::<Alignment>(player).unwrap().0, -3);
}

#[test]
fn challenge_the_balance_is_idempotent() {
    let mut w = spirits_world(0);
    spirits::challenge_the_balance(&mut w);
    spirits::challenge_the_balance(&mut w);
    let count = w
        .resource::<GameLog>()
        .unread
        .iter()
        .filter(|l| l.text.contains("challenge the balance"))
        .count();
    assert_eq!(count, 1);
}

// ---------------------------------------------------------------------------
// The "choose one of three" offer menu.
// ---------------------------------------------------------------------------

#[test]
fn rolled_offers_are_three_distinct_options() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    for options in [
        spirits::roll_spell_offer(&mut w, player),
        spirits::roll_weapon_offer(&mut w),
        spirits::roll_armor_offer(&mut w),
        spirits::roll_ring_offer(&mut w),
    ] {
        assert_eq!(options.len(), 3);
        let names: Vec<&str> = options.iter().map(|o| o.display_name()).collect();
        let mut deduped = names.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), names.len(), "duplicate offered: {names:?}");
    }
}

#[test]
fn confirming_a_spell_offer_teaches_it() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let options = spirits::roll_spell_offer(&mut w, player);
    let OfferOption::Spell(effect) = options[0] else {
        panic!("not a spell offer");
    };
    spirits::confirm_offer(&mut w, player, options[0]);
    assert!(w.get::<Spellset>(player).unwrap().slots.contains(&effect));
}

#[test]
fn a_full_spellset_declines_the_offer_instead_of_overflowing() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Spellset>(player).unwrap().slots = vec![
        SpellEffect::Sting,
        SpellEffect::Thunderbolt,
        SpellEffect::Cure,
        SpellEffect::Heal,
    ];
    let before = w.get::<Spellset>(player).unwrap().slots.clone();
    spirits::confirm_offer(&mut w, player, OfferOption::Spell(SpellEffect::Bide));
    assert_eq!(w.get::<Spellset>(player).unwrap().slots, before);
    assert!(logged(&w, "nowhere to sit"));
}

#[test]
fn confirming_a_weapon_offer_gives_an_identified_item_in_the_pack() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let options = spirits::roll_weapon_offer(&mut w);
    spirits::confirm_offer(&mut w, player, options[0]);
    let items = &w.get::<Backpack>(player).unwrap().items;
    assert_eq!(items.len(), 1);
    assert!(w.get::<KnownQuality>(items[0]).is_some());
    assert!(w.get::<Position>(items[0]).is_none());
}

#[test]
fn a_full_pack_leaves_the_offered_item_on_the_ground() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let filler: Vec<Entity> = (0..9)
        .map(|_| {
            w.spawn(Name {
                what: "filler".into(),
            })
            .id()
        })
        .collect();
    w.get_mut::<Backpack>(player).unwrap().items = filler;
    let options = spirits::roll_ring_offer(&mut w);
    spirits::confirm_offer(&mut w, player, options[0]);
    assert_eq!(w.get::<Backpack>(player).unwrap().items.len(), 9);
}

#[test]
fn opening_the_menu_with_no_options_is_a_no_op() {
    let mut w = spirits_world(0);
    spirits::open_offer_menu(&mut w, Vec::new(), None, false);
    assert!(!w.resource::<OfferMenu>().open);
}

#[test]
fn opening_the_menu_stores_the_rolled_options() {
    let mut w = spirits_world(0);
    let options = spirits::roll_armor_offer(&mut w);
    let count = options.len();
    spirits::open_offer_menu(&mut w, options, None, false);
    let menu = w.resource::<OfferMenu>();
    assert!(menu.open);
    assert_eq!(menu.selected, 0);
    assert_eq!(menu.options.len(), count);
}

// ---------------------------------------------------------------------------
// The barter menu.
// ---------------------------------------------------------------------------

fn spawn_demon(w: &mut World, items: Vec<Entity>) -> Entity {
    w.spawn((
        Name {
            what: "yellow demon".into(),
        },
        Faction::Spirits,
        Backpack { items },
    ))
    .id()
}

fn dagger(w: &mut World) -> Entity {
    w.spawn(Name {
        what: "dagger".into(),
    })
    .id()
}

#[test]
fn opening_an_item_barter_loads_both_columns() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let my_item = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(my_item);
    let their_item = dagger(&mut w);
    let demon = spawn_demon(&mut w, vec![their_item]);

    spirits::open_item_barter(&mut w, player, demon);

    let menu = w.resource::<BarterMenu>();
    assert!(menu.open);
    assert_eq!(menu.demon, Some(demon));
    assert_eq!(menu.player_side, vec![Tradeable::Item(my_item)]);
    assert_eq!(menu.demon_side, vec![Tradeable::Item(their_item)]);
    assert!(menu.player_selected.is_empty());
    assert!(menu.demon_selected.is_empty());
}

#[test]
fn toggling_a_row_stages_and_unstages_it() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let my_item = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(my_item);
    let demon = spawn_demon(&mut w, Vec::new());
    spirits::open_item_barter(&mut w, player, demon);

    spirits::toggle_barter_selection(&mut w);
    assert_eq!(
        w.resource::<BarterMenu>().player_selected,
        vec![Tradeable::Item(my_item)]
    );

    spirits::toggle_barter_selection(&mut w);
    assert!(w.resource::<BarterMenu>().player_selected.is_empty());
}

#[test]
fn confirming_swaps_only_what_was_staged_and_poofs_the_demon() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let given = dagger(&mut w);
    let kept = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items = vec![given, kept];
    let taken = dagger(&mut w);
    let left_behind = dagger(&mut w);
    let demon = spawn_demon(&mut w, vec![taken, left_behind]);

    spirits::open_item_barter(&mut w, player, demon);
    {
        let mut menu = w.resource_mut::<BarterMenu>();
        menu.player_selected = vec![Tradeable::Item(given)];
        menu.demon_selected = vec![Tradeable::Item(taken)];
    }
    spirits::confirm_barter(&mut w, player);

    let pack = &w.get::<Backpack>(player).unwrap().items;
    assert!(pack.contains(&kept), "kept item vanished");
    assert!(pack.contains(&taken), "traded-for item never arrived");
    assert!(!pack.contains(&given), "given-up item stayed in the pack");

    assert!(w.get_entity(given).is_none(), "given item should be gone");
    assert!(w.get_entity(demon).is_none(), "demon should have poofed");
    assert!(
        w.get_entity(left_behind).is_none(),
        "unselected demon loot should vanish with it"
    );
    assert!(logged(&w, "satisfied"));
    assert!(!w.resource::<BarterMenu>().open);
}

#[test]
fn cancelling_moves_nothing() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let my_item = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(my_item);
    let their_item = dagger(&mut w);
    let demon = spawn_demon(&mut w, vec![their_item]);
    spirits::open_item_barter(&mut w, player, demon);
    w.resource_mut::<BarterMenu>().player_selected = vec![Tradeable::Item(my_item)];

    spirits::cancel_barter(&mut w);

    assert!(!w.resource::<BarterMenu>().open);
    assert_eq!(w.get::<Backpack>(player).unwrap().items, vec![my_item]);
    assert!(
        w.get_entity(demon).is_some(),
        "cancelling should not kill the demon"
    );
    assert!(w.get_entity(their_item).is_some());
}

#[test]
fn an_item_barter_with_an_empty_pack_grunts_instead_of_opening() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let their_item = dagger(&mut w);
    let demon = spawn_demon(&mut w, vec![their_item]);

    spirits::open_item_barter(&mut w, player, demon);

    assert!(!w.resource::<BarterMenu>().open);
    assert!(logged(&w, "grunts"));
}

#[test]
fn a_spell_barter_with_an_empty_spellset_grunts_instead_of_opening() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let demon = spawn_demon(&mut w, Vec::new());

    spirits::open_spell_barter(&mut w, player, demon, &[SpellEffect::Cure]);

    assert!(!w.resource::<BarterMenu>().open);
    assert!(logged(&w, "grunts"));
}

#[test]
fn spell_barter_trades_a_known_spell_for_an_offered_one() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Spellset>(player).unwrap().slots = vec![SpellEffect::Sting];
    let demon = spawn_demon(&mut w, Vec::new());

    spirits::open_spell_barter(&mut w, player, demon, &[SpellEffect::Cure]);
    {
        let mut menu = w.resource_mut::<BarterMenu>();
        menu.player_selected = vec![Tradeable::Spell(SpellEffect::Sting)];
        menu.demon_selected = vec![Tradeable::Spell(SpellEffect::Cure)];
    }
    spirits::confirm_barter(&mut w, player);

    let slots = &w.get::<Spellset>(player).unwrap().slots;
    assert_eq!(slots, &vec![SpellEffect::Cure]);
}

#[test]
fn the_demons_side_only_shows_as_many_rows_as_you_can_pay_for() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let mine = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(mine);
    let theirs: Vec<Entity> = (0..3).map(|_| dagger(&mut w)).collect();
    let demon = spawn_demon(&mut w, theirs.clone());

    spirits::open_item_barter(&mut w, player, demon);

    let menu = w.resource::<BarterMenu>();
    assert_eq!(menu.demon_visible(), &[Tradeable::Item(theirs[0])]);
}

#[test]
fn a_hidden_demon_row_cannot_be_staged() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let mine = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(mine);
    let theirs: Vec<Entity> = (0..3).map(|_| dagger(&mut w)).collect();
    let demon = spawn_demon(&mut w, theirs);
    spirits::open_item_barter(&mut w, player, demon);
    {
        let mut menu = w.resource_mut::<BarterMenu>();
        menu.column = BarterColumn::Demon;
        menu.cursor = 1;
    }

    spirits::toggle_barter_selection(&mut w);

    assert!(w.resource::<BarterMenu>().demon_selected.is_empty());
}

#[test]
fn a_spawned_yellow_demon_carries_a_stocked_pack() {
    use models::constants::spirits::{BARTER_STOCK_MAX, BARTER_STOCK_MIN};
    for seed in 0..20 {
        let mut w = spirits_world(seed);
        let demon = spawn_monster(
            &mut w,
            MonsterDef::named("yellow demon"),
            Position { x: 5, y: 5 },
        );
        let stock = w.get::<Backpack>(demon).unwrap().items.len();
        assert!((BARTER_STOCK_MIN..=BARTER_STOCK_MAX).contains(&stock));
    }
}

#[test]
fn a_demons_pack_never_holds_coins() {
    for seed in 0..200 {
        let mut w = spirits_world(seed);
        let demon = spawn_monster(
            &mut w,
            MonsterDef::named("yellow demon"),
            Position { x: 5, y: 5 },
        );
        let items = w.get::<Backpack>(demon).unwrap().items.clone();
        assert!(items.iter().all(|&i| w.get::<Pickup>(i).is_none()));
    }
}

#[test]
fn the_sphynx_offers_a_stocked_range_of_spells() {
    use models::constants::spirits::{BARTER_STOCK_MAX, BARTER_STOCK_MIN};
    for seed in 0..20 {
        let mut w = spirits_world(seed);
        let player = spawn_player(&mut w, 6);
        w.get_mut::<Spellset>(player).unwrap().slots = vec![SpellEffect::Sting];
        let sphynx = spawn_monster(&mut w, MonsterDef::named("sphynx"), Position { x: 5, y: 5 });
        resolve_attack(&mut w, player, sphynx);
        let offered = w.resource::<BarterMenu>().demon_side.len();
        assert!((BARTER_STOCK_MIN..=BARTER_STOCK_MAX).contains(&offered));
    }
}

// ---------------------------------------------------------------------------
// The nine species' bestiary rows and event fns.
// ---------------------------------------------------------------------------

#[test]
fn every_spirit_row_spawns_peaceful_and_wandering() {
    let mut w = spirits_world(0);
    for name in [
        "yellow demon",
        "red demon",
        "blue demon",
        "pink demon",
        "angel",
        "sphynx",
        "sylphid",
        "salamander",
        "undyne",
        "gnome",
    ] {
        let def = MonsterDef::named(name);
        let e = spawn_monster(&mut w, def, Position { x: 5, y: 5 });
        assert_eq!(
            w.get::<Faction>(e).copied(),
            Some(Faction::Spirits),
            "{name}"
        );
        assert!(
            matches!(
                w.get::<Mob>(e).map(|m| m.movement_type),
                Some(MovementType::Confused)
            ),
            "{name}"
        );
        w.despawn(e);
    }
}

/// Spawns the spirit named `name` and talks to it (a peaceful melee).
fn talk_to(w: &mut World, player: Entity, name: &str) -> Entity {
    let spirit = spawn_monster(w, MonsterDef::named(name), Position { x: 5, y: 5 });
    resolve_attack(w, player, spirit);
    spirit
}

fn offered(w: &World) -> Vec<OfferOption> {
    w.resource::<OfferMenu>().options.clone()
}

#[test]
fn the_red_demon_sells_a_weapon_an_armor_and_a_ring_for_max_hp() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = talk_to(&mut w, player, "red demon");
    let menu = w.resource::<OfferMenu>();
    assert!(menu.open);
    assert!(menu.priced);
    assert_eq!(menu.source, Some(spirit));
    let options = offered(&w);
    assert!(matches!(options[0], OfferOption::Weapon(_)));
    assert!(matches!(options[1], OfferOption::Armor(_)));
    assert!(matches!(options[2], OfferOption::Ring(_)));
    assert!(options.iter().all(|o| o.price() == Some(Price::MaxHp(3))));
}

#[test]
fn buying_from_the_red_demon_costs_max_hp_and_poofs_them() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Fighter>(player).unwrap().max_hp = 20;
    w.get_mut::<Fighter>(player).unwrap().hp = 19;
    let spirit = talk_to(&mut w, player, "red demon");
    let choice = offered(&w)[1];
    spirits::confirm_offer(&mut w, player, choice);
    let f = w.get::<Fighter>(player).unwrap();
    assert_eq!((f.hp, f.max_hp), (17, 17));
    assert_eq!(w.get::<Backpack>(player).unwrap().items.len(), 1);
    assert!(w.get_entity(spirit).is_none());
    assert!(!w.resource::<OfferMenu>().open);
}

/// Asserts `spirit` refused `player`: a grunt, no menu, still standing, and
/// no pull on their alignment, since nothing was dealt.
fn refused(w: &World, player: Entity, spirit: Entity) {
    assert!(logged(w, "grunts"));
    assert_eq!(w.get::<Alignment>(player).unwrap().0, 0);
    assert!(!w.resource::<OfferMenu>().open);
    assert!(!w.resource::<BarterMenu>().open);
    assert!(w.get_entity(spirit).is_some(), "a refusal is no poof");
}

#[test]
fn the_red_demon_refuses_a_player_short_of_max_hp() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Fighter>(player).unwrap().max_hp = 2;
    w.get_mut::<Fighter>(player).unwrap().hp = 2;
    let spirit = talk_to(&mut w, player, "red demon");
    refused(&w, player, spirit);
    assert_eq!(w.get::<Fighter>(player).unwrap().max_hp, 2);
}

#[test]
fn the_gnome_refuses_a_player_with_no_max_ma() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.entity_mut(player).insert(Magic {
        points: 0,
        max_points: 0,
    });
    let spirit = talk_to(&mut w, player, "gnome");
    refused(&w, player, spirit);
}

#[test]
fn the_yellow_demon_refuses_an_empty_pack() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = talk_to(&mut w, player, "yellow demon");
    refused(&w, player, spirit);
}

#[test]
fn the_sphynx_refuses_an_empty_spellset() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = talk_to(&mut w, player, "sphynx");
    refused(&w, player, spirit);
}

#[test]
fn paying_the_red_demon_your_last_max_hp_kills_you() {
    let mut w = spirits_world(0);
    // Dying leaves a corpse.
    w.init_resource::<BloodStains>();
    w.init_resource::<Corpses>();
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Fighter>(player).unwrap().max_hp = 3;
    talk_to(&mut w, player, "red demon");
    let choice = offered(&w)[2];
    spirits::confirm_offer(&mut w, player, choice);
    assert_eq!(w.get::<Fighter>(player).unwrap().hp, 0);
    assert!(w.resource::<Ending>().player_dead);
}

#[test]
fn the_gnome_sells_a_scroll_a_potion_and_a_wand_for_max_ma() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.entity_mut(player).insert(Magic {
        points: 2,
        max_points: 2,
    });
    talk_to(&mut w, player, "gnome");
    assert!(w.resource::<OfferMenu>().priced);
    let options = offered(&w);
    assert!(matches!(options[0], OfferOption::Scroll(_)));
    assert!(matches!(options[1], OfferOption::Potion(_)));
    assert!(matches!(options[2], OfferOption::Wand(_)));
    let prices: Vec<_> = options.iter().map(OfferOption::price).collect();
    assert_eq!(
        prices,
        [
            Some(Price::MaxMa(1)),
            Some(Price::MaxMa(1)),
            Some(Price::MaxMa(2))
        ]
    );
    assert!(MonsterDef::named("gnome").spells.is_empty());
}

#[test]
fn buying_a_wand_from_the_gnome_costs_two_max_ma() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.entity_mut(player).insert(Magic {
        points: 3,
        max_points: 3,
    });
    let spirit = talk_to(&mut w, player, "gnome");
    let choice = offered(&w)[2];
    spirits::confirm_offer(&mut w, player, choice);
    let m = w.get::<Magic>(player).unwrap();
    assert_eq!((m.points, m.max_points), (1, 1));
    assert_eq!(w.get::<Backpack>(player).unwrap().items.len(), 1);
    assert!(w.get_entity(spirit).is_none());
}

#[test]
fn the_gnome_refuses_a_player_short_of_max_ma() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.entity_mut(player).insert(Magic {
        points: 1,
        max_points: 1,
    });
    let spirit = talk_to(&mut w, player, "gnome");
    let wand = offered(&w)[2];
    spirits::confirm_offer(&mut w, player, wand);
    assert_eq!(w.get::<Magic>(player).unwrap().max_points, 1);
    assert!(w.get_entity(spirit).is_some());
    assert!(w.resource::<OfferMenu>().open);
    // The scroll is still within reach.
    let scroll = offered(&w)[0];
    spirits::confirm_offer(&mut w, player, scroll);
    assert_eq!(w.get::<Magic>(player).unwrap().max_points, 0);
    assert!(w.get_entity(spirit).is_none());
}

#[test]
fn a_free_offer_costs_nothing() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    talk_to(&mut w, player, "sylphid");
    assert!(!w.resource::<OfferMenu>().priced);
    let choice = offered(&w)[0];
    spirits::confirm_offer(&mut w, player, choice);
    assert_eq!(w.get::<Fighter>(player).unwrap().max_hp, 20);
}

#[test]
fn the_yellow_demon_opens_an_item_barter() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let item = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("yellow demon"),
        Position { x: 5, y: 5 },
    );
    resolve_attack(&mut w, player, spirit);
    let menu = w.resource::<BarterMenu>();
    assert!(menu.open);
    assert_eq!(menu.demon, Some(spirit));
}

#[test]
fn the_blue_demon_opens_a_spell_offer() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("blue demon"),
        Position { x: 5, y: 5 },
    );
    resolve_attack(&mut w, player, spirit);
    let menu = w.resource::<OfferMenu>();
    assert!(menu.open);
    assert_eq!(menu.source, Some(spirit));
    assert!(
        menu.options
            .iter()
            .all(|o| matches!(o, OfferOption::Spell(_)))
    );
}

fn wear(w: &mut World, player: Entity, slot: Slot) -> Entity {
    let item = dagger(w);
    w.entity_mut(item).insert(Equipped {
        by: Some(player),
        slot,
    });
    w.get_mut::<Backpack>(player).unwrap().items.push(item);
    item
}

#[test]
fn the_pink_demon_destroys_every_piece_you_wear_cursed_or_not() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let sword = wear(&mut w, player, Slot::Hand);
    let cursed = wear(&mut w, player, Slot::Body);
    w.entity_mut(cursed).insert(Curse);
    let pocketed = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(pocketed);
    talk_to(&mut w, player, "pink demon");
    assert!(w.get_entity(sword).is_none());
    assert!(w.get_entity(cursed).is_none());
    assert_eq!(w.get::<Backpack>(player).unwrap().items, vec![pocketed]);
}

#[test]
fn the_pink_demon_refuses_an_unequipped_player() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let pocketed = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items.push(pocketed);
    let spirit = talk_to(&mut w, player, "pink demon");
    refused(&w, player, spirit);
    assert!(
        w.get_entity(pocketed).is_some(),
        "the pack is not their business"
    );
    assert!(!w.resource::<SpiritsHostile>().0);
}

#[test]
fn a_pink_demon_who_will_not_join_vanishes() {
    // One piece is 25%: some seed refuses.
    let refusal = (0..40).find_map(|seed| {
        let mut w = spirits_world(seed);
        let player = spawn_player(&mut w, 6);
        wear(&mut w, player, Slot::Hand);
        let spirit = talk_to(&mut w, player, "pink demon");
        w.get_entity(spirit).is_none().then_some(w)
    });
    let w = refusal.expect("no seed in 40 refused at 25% odds");
    assert!(logged(&w, "vanishes"));
    let player = w
        .iter_entities()
        .find(|e| e.contains::<Player>())
        .unwrap()
        .id();
    assert_eq!(alignment(&w, player), -1, "vanishing is a poof");
    assert!(!w.resource::<SpiritsHostile>().0);
}

#[test]
fn four_pieces_destroyed_always_win_the_pink_demon_over() {
    for seed in 0..20 {
        let mut w = spirits_world(seed);
        let player = spawn_player(&mut w, 6);
        for slot in [Slot::Hand, Slot::Body, Slot::Finger, Slot::Finger] {
            wear(&mut w, player, slot);
        }
        let spirit = talk_to(&mut w, player, "pink demon");
        assert!(w.get::<Helper>(spirit).is_some(), "seed {seed}");
        assert_eq!(alignment(&w, player), -1, "joining pulls alignment");
    }
}

#[test]
fn the_angel_halves_hp_cancels_effects_and_poofs() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Fighter>(player).unwrap().hp = 21;
    w.entity_mut(player).insert(Bided);
    let spirit = spawn_monster(&mut w, MonsterDef::named("angel"), Position { x: 5, y: 5 });

    resolve_attack(&mut w, player, spirit);

    assert_eq!(w.get::<Fighter>(player).unwrap().hp, 10);
    assert!(
        w.get::<Bided>(player).is_none(),
        "faith cancels active effects"
    );
    assert!(w.get::<TestOfFaithLedger>(player).is_some());
    assert!(w.get_entity(spirit).is_none(), "the angel poofs");
    assert!(logged(&w, "Tests your faith"));
}

#[test]
fn the_test_of_faith_blesses_only_equipped_gear() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.entity_mut(player).insert(TestOfFaithLedger);

    let sword = dagger(&mut w);
    w.entity_mut(sword).insert(Equipped {
        by: Some(player),
        slot: Slot::Hand,
    });
    let pocketed = dagger(&mut w);
    w.get_mut::<Backpack>(player).unwrap().items = vec![sword, pocketed];

    spirits::apply_test_of_faith(&mut w, player);

    assert_eq!(w.get::<PowerBonus>(sword).map(|b| b.0), Some(3));
    assert!(w.get::<KnownQuality>(sword).is_some());
    assert!(
        w.get::<PowerBonus>(pocketed).is_none(),
        "an unequipped item is untouched"
    );
    assert!(w.get::<TestOfFaithLedger>(player).is_none());
}

#[test]
fn the_test_of_faith_is_a_no_op_without_a_pending_ledger() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let sword = dagger(&mut w);
    w.entity_mut(sword).insert(Equipped {
        by: Some(player),
        slot: Slot::Hand,
    });
    w.get_mut::<Backpack>(player).unwrap().items = vec![sword];

    spirits::apply_test_of_faith(&mut w, player);

    assert!(w.get::<PowerBonus>(sword).is_none());
}

#[test]
fn the_sphynx_opens_a_spell_barter() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    w.get_mut::<Spellset>(player).unwrap().slots = vec![SpellEffect::Sting];
    let spirit = spawn_monster(&mut w, MonsterDef::named("sphynx"), Position { x: 5, y: 5 });
    resolve_attack(&mut w, player, spirit);
    let menu = w.resource::<BarterMenu>();
    assert!(menu.open);
    assert_eq!(menu.demon, Some(spirit));
    assert!(
        menu.demon_side
            .iter()
            .all(|t| matches!(t, Tradeable::Spell(_)))
    );
}

#[test]
fn sylphid_salamander_and_undyne_open_their_respective_offers() {
    fn is_weapon(o: &OfferOption) -> bool {
        matches!(o, OfferOption::Weapon(_))
    }
    fn is_armor(o: &OfferOption) -> bool {
        matches!(o, OfferOption::Armor(_))
    }
    fn is_ring(o: &OfferOption) -> bool {
        matches!(o, OfferOption::Ring(_))
    }
    type Case = (&'static str, fn(&OfferOption) -> bool);
    let cases: [Case; 3] = [
        ("sylphid", is_weapon),
        ("salamander", is_armor),
        ("undyne", is_ring),
    ];
    for (name, is_right_kind) in cases {
        let mut w = spirits_world(0);
        let player = spawn_player(&mut w, 6);
        let spirit = spawn_monster(&mut w, MonsterDef::named(name), Position { x: 5, y: 5 });
        resolve_attack(&mut w, player, spirit);
        let menu = w.resource::<OfferMenu>();
        assert!(menu.open, "{name}");
        assert!(menu.options.iter().all(is_right_kind), "{name}");
    }
}

#[test]
fn a_spirit_killed_poofs_too() {
    let mut w = spirits_world(0);
    w.init_resource::<BloodStains>();
    w.init_resource::<Corpses>();
    w.resource_mut::<SpiritsHostile>().0 = true;
    let player = spawn_player(&mut w, 200);
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("red demon"),
        Position { x: 2, y: 1 },
    );
    w.get_mut::<Fighter>(spirit).unwrap().hp = 1;
    for _ in 0..50 {
        if w.get_entity(spirit).is_none() {
            break;
        }
        resolve_attack(&mut w, player, spirit);
    }
    assert!(w.get_entity(spirit).is_none(), "never landed the kill");
    assert_eq!(alignment(&w, player), -1);
}

/// Every spirit shrugs off fire and cold, so a blast of either draws no
/// blood and angers nobody.
#[test]
fn fire_and_cold_neither_hurt_nor_anger_a_spirit() {
    for element in [Element::Fire, Element::Cold] {
        let mut w = spirits_world(0);
        let spirit = spawn_monster(&mut w, MonsterDef::named("gnome"), Position { x: 5, y: 5 });
        assert_eq!(
            apply_hit(&mut w, spirit, Hit::elemental(5, element), None),
            0
        );
        assert!(!w.resource::<SpiritsHostile>().0);
    }
}

/// A whole generated floor, for paths that need the map and its overlays.
fn game_world(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<Ending>();
    w.init_resource::<PlayerTempo>();
    w.init_resource::<OfferMenu>();
    w.init_resource::<BarterMenu>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    (w, player)
}

/// Joins the player as a pink demon Helper, with alignment back at 0.
fn pink_ally(w: &mut World, player: Entity) -> Entity {
    for slot in [Slot::Hand, Slot::Body, Slot::Finger, Slot::Finger] {
        wear(w, player, slot);
    }
    let pink = talk_to(w, player, "pink demon");
    assert!(w.get::<Helper>(pink).is_some());
    w.get_mut::<Alignment>(player).unwrap().0 = 0;
    pink
}

#[test]
fn wounding_a_pink_demon_ally_angers_nobody() {
    let (mut w, player) = game_world(0);
    let pink = pink_ally(&mut w, player);
    apply_hit(&mut w, pink, Hit::physical(3), None);
    assert!(!w.resource::<SpiritsHostile>().0);
}

#[test]
fn a_pink_demon_ally_exploding_for_a_new_helper_leaves_alignment_be() {
    let (mut w, player) = game_world(0);
    let pink = pink_ally(&mut w, player);
    let orc = monster::plain_monster(&mut w, "orc", Position { x: 3, y: 3 });
    recruit(&mut w, orc);
    assert!(w.get_entity(pink).is_none(), "the old Helper exploded");
    assert_eq!(alignment(&w, player), 0);
    assert!(!w.resource::<SpiritsHostile>().0);
}

/// Every spirit is born with two random boons on top of what its row lends,
/// never the same one twice.
#[test]
fn every_spirit_is_born_with_two_distinct_random_boons() {
    for def in BESTIARY.iter().filter(|d| d.spirit_kind.is_some()) {
        for seed in 0..8 {
            let mut w = spirits_world(seed);
            let e = spawn_monster(&mut w, def, Position { x: 5, y: 5 });
            // Born with, not lent by gear the row happened to roll.
            let mut held: Vec<&str> = w
                .get::<Effects>(e)
                .unwrap()
                .0
                .iter()
                .filter(|h| matches!(h.lifetime, Lifetime::Permanent))
                .map(|h| h.id)
                .collect();
            held.sort();
            held.dedup();
            let fixed = def.grants.iter().filter_map(|g| g.effect_id()).count();
            assert_eq!(held.len(), fixed + 2, "{} seed {seed}: {held:?}", def.name);
        }
    }
}
