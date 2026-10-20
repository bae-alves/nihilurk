use super::*;

fn temp_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "nihilurk-bones-unit-{}-{tag}.sav",
            std::process::id()
        ))
        .to_str()
        .unwrap()
        .to_string()
}

fn world_with_player() -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(PlayerName { what: "BAE".into() });
    let player = w.spawn((Player, Backpack { items: Vec::new() })).id();
    (w, player)
}

#[test]
fn a_dead_name_with_an_escape_comes_back_without_it() {
    let path = temp_path("escape");
    let _ = std::fs::remove_file(&path);
    let (mut w, _) = world_with_player();
    w.insert_resource(PlayerName {
        what: "BAE\x1b[31m".into(),
    });

    deposit_to(&mut w, &path).unwrap();
    let bones = take_from(&path).expect("a bones file was written");

    assert_eq!(bones.name, "BAE[31m");
}

#[test]
fn deposit_then_take_round_trips_equipped_and_loose_items() {
    let path = temp_path("roundtrip");
    let _ = std::fs::remove_file(&path);
    let (mut w, player) = world_with_player();

    let sword = w
        .spawn((
            Name {
                what: "long sword".into(),
            },
            Equipped {
                by: Some(player),
                slot: Slot::Hand,
            },
            PowerBonus(3),
        ))
        .id();
    let potion = w
        .spawn(Name {
            what: "potion of healing".into(),
        })
        .id();
    w.get_mut::<Backpack>(player).unwrap().items.push(potion);

    deposit_to(&mut w, &path).unwrap();
    let bones = take_from(&path).expect("a bones file was written");
    let _ = std::fs::remove_file(&path);

    assert_eq!(bones.name, "BAE");
    let items: Vec<_> = bones.items().collect();
    assert!(
        items
            .iter()
            .any(|&(name, slot, power, ..)| name == "long sword"
                && slot == Some(Slot::Hand)
                && power == 3),
        "the worn sword and its enchantment came back: {items:?}"
    );
    assert!(
        items
            .iter()
            .any(|&(name, slot, ..)| name == "potion of healing" && slot.is_none()),
        "the loose potion came back unworn: {items:?}"
    );
    let _ = sword;
}

#[test]
fn a_ring_of_healths_plus_survives_the_bones_file() {
    let path = temp_path("ring-hp");
    let _ = std::fs::remove_file(&path);
    let (mut w, player) = world_with_player();
    let ring = w
        .spawn((
            Name {
                what: "ring of health".into(),
            },
            MaxHpBonus(-3),
        ))
        .id();
    w.get_mut::<Backpack>(player).unwrap().items.push(ring);

    deposit_to(&mut w, &path).unwrap();
    let bones = take_from(&path).expect("a bones file was written");
    let _ = std::fs::remove_file(&path);

    let hp: Vec<i32> = bones.items().map(|(.., hp, _)| hp).collect();
    assert_eq!(hp, vec![-3]);
}

#[test]
fn take_deletes_the_file_so_a_second_take_is_none() {
    let path = temp_path("single-use");
    let _ = std::fs::remove_file(&path);
    let (mut w, _player) = world_with_player();

    deposit_to(&mut w, &path).unwrap();
    assert!(take_from(&path).is_some());
    assert!(
        take_from(&path).is_none(),
        "a second take on the same depth finds nothing left"
    );
}

#[test]
fn depositing_again_overwrites_rather_than_appends() {
    let path = temp_path("overwrite");
    let _ = std::fs::remove_file(&path);
    let (mut w, player) = world_with_player();

    let first = w
        .spawn(Name {
            what: "dagger".into(),
        })
        .id();
    w.get_mut::<Backpack>(player).unwrap().items.push(first);
    deposit_to(&mut w, &path).unwrap();

    w.get_mut::<Backpack>(player).unwrap().items.clear();
    let second = w
        .spawn(Name {
            what: "mace".into(),
        })
        .id();
    w.get_mut::<Backpack>(player).unwrap().items.push(second);
    deposit_to(&mut w, &path).unwrap();

    let bones = take_from(&path).expect("the overwritten file is still there");
    let _ = std::fs::remove_file(&path);
    let names: Vec<&str> = bones.items().map(|(name, ..)| name).collect();
    assert_eq!(names, vec!["mace"], "only the latest death's loot survives");
}

#[test]
fn a_disabled_bones_flag_deposits_nothing() {
    let depth = 255;
    let dir = std::env::temp_dir().join(format!("nihilurk-bones-unit-{}-off", std::process::id()));
    let path = path_for(&dir, depth);
    let _ = std::fs::remove_file(&path);
    let (mut w, player) = world_with_player();
    w.insert_resource(Bones {
        enabled: false,
        dir: dir.clone(),
    });
    w.insert_resource(Depth { what: depth });

    let item = w
        .spawn(Name {
            what: "dagger".into(),
        })
        .id();
    w.get_mut::<Backpack>(player).unwrap().items.push(item);

    deposit(&mut w).unwrap();

    let result = take(&dir, depth);
    let _ = std::fs::remove_file(&path);
    assert!(
        result.is_none(),
        "the flag being off means deposit() itself never wrote a file"
    );
}
