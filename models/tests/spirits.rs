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
/// actually *fought* by a sword. Only a ranged/thrown hit
/// (`items::throwing::strike_victim`, exercised in `missiles.rs`) can ever
/// reach `spirits::on_direct_hit` with real damage. See
/// `combat::resolve_attack`'s spirit branch.
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

#[test]
fn melee_on_a_peaceful_spirit_still_shifts_alignment_by_its_kind() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_spirit(&mut w, 100);
    w.entity_mut(spirit).insert(SpiritKind::Cacodaemon);
    resolve_attack(&mut w, player, spirit);
    assert_eq!(w.get::<Alignment>(player).unwrap().0, -1);
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

#[test]
fn a_monster_hitting_a_spirit_does_not_flip_the_flag() {
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
    assert!(!w.resource::<SpiritsHostile>().0);
}

#[test]
fn zero_damage_never_flips_the_flag() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_spirit(&mut w, 100);
    spirits::on_direct_hit(&mut w, player, spirit, 0);
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
    spirits::open_offer_menu(&mut w, Vec::new(), None);
    assert!(!w.resource::<OfferMenu>().open);
}

#[test]
fn opening_the_menu_stores_the_rolled_options() {
    let mut w = spirits_world(0);
    let options = spirits::roll_armor_offer(&mut w);
    let count = options.len();
    spirits::open_offer_menu(&mut w, options, None);
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

#[test]
fn the_gnome_carries_its_wand_throwing_grant() {
    let mut w = spirits_world(0);
    let def = MonsterDef::named("gnome");
    let e = spawn_monster(&mut w, def, Position { x: 5, y: 5 });
    assert!(w.get::<ThrowsWands>(e).is_some());
    assert!(def.spirit_event.is_none(), "gnome has no melee event");
}

#[test]
fn the_red_demon_only_grunts() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("red demon"),
        Position { x: 5, y: 5 },
    );
    resolve_attack(&mut w, player, spirit);
    assert!(logged(&w, "grunts"));
    assert!(w.get_entity(spirit).is_some(), "the red demon never poofs");
    assert!(!w.resource::<OfferMenu>().open);
    assert!(!w.resource::<BarterMenu>().open);
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

#[test]
fn an_unequipped_player_always_makes_the_pink_demon_turn() {
    let mut w = spirits_world(0);
    let player = spawn_player(&mut w, 6);
    let spirit = spawn_monster(
        &mut w,
        MonsterDef::named("pink demon"),
        Position { x: 5, y: 5 },
    );
    resolve_attack(&mut w, player, spirit);
    assert_eq!(w.get::<Faction>(spirit).copied(), Some(Faction::Monster));
    assert!(matches!(
        w.get::<Mob>(spirit).map(|m| m.movement_type),
        Some(MovementType::Chase)
    ));
    assert!(logged(&w, "turns on you"));
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
