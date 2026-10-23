//! The level feeling: how crowded and how rich a floor is against what its
//! depth usually holds, said once on arriving by stairs.

use crate::common;
use crate::common::monster;

use bevy_ecs::prelude::*;
use common::fresh_run;
use models::Color;
use models::*;
use monster::plain_monster;

fn clear_floor(w: &mut World) {
    let doomed: Vec<Entity> = w
        .query_filtered::<Entity, (With<Position>, Without<Player>, Without<Helper>)>()
        .iter(w)
        .filter(|&e| w.get::<Mob>(e).is_some() || w.get::<Item>(e).is_some())
        .collect();
    for e in doomed {
        w.despawn(e);
    }
}

fn crowd(w: &mut World, n: usize) {
    for i in 0..n {
        plain_monster(w, "rat", Position { x: i as u16, y: 1 });
    }
}

fn haul(w: &mut World, n: usize) {
    for i in 0..n {
        w.spawn((Item, Position { x: i as u16, y: 2 }));
    }
}

#[test]
fn a_floor_full_of_monsters_feels_dangerous() {
    let (mut w, _) = fresh_run(7);
    clear_floor(&mut w);
    crowd(&mut w, 40);
    assert_eq!(level_feeling(&mut w).danger, Some(Danger::Dangerous));
}

#[test]
fn a_floor_with_no_monsters_feels_safe() {
    let (mut w, _) = fresh_run(7);
    clear_floor(&mut w);
    assert_eq!(level_feeling(&mut w).danger, Some(Danger::Safe));
}

#[test]
fn a_floor_full_of_items_feels_rich() {
    let (mut w, _) = fresh_run(7);
    clear_floor(&mut w);
    haul(&mut w, 40);
    assert_eq!(level_feeling(&mut w).riches, Some(Riches::Rich));
}

#[test]
fn a_floor_with_no_items_feels_poor() {
    let (mut w, _) = fresh_run(7);
    clear_floor(&mut w);
    assert_eq!(level_feeling(&mut w).riches, Some(Riches::Poor));
}

#[test]
fn a_helper_is_not_a_threat() {
    let (mut w, _) = fresh_run(7);
    clear_floor(&mut w);
    let pal = plain_monster(&mut w, "dog", Position { x: 3, y: 3 });
    w.entity_mut(pal).insert(Helper);
    assert_eq!(level_feeling(&mut w).danger, Some(Danger::Safe));
}

#[test]
fn each_line_speaks_on_some_floors_and_none_on_most() {
    let mut tally = [0u32; 4];
    let mut floors = 0u32;
    for seed in 1..=100 {
        let (mut w, p) = fresh_run(seed);
        loop {
            let f = level_feeling(&mut w);
            floors += 1;
            tally[0] += (f.danger == Some(Danger::Safe)) as u32;
            tally[1] += (f.danger == Some(Danger::Dangerous)) as u32;
            tally[2] += (f.riches == Some(Riches::Poor)) as u32;
            tally[3] += (f.riches == Some(Riches::Rich)) as u32;
            let Some(down) = w
                .resource::<Map>()
                .tiles
                .iter()
                .position(|&t| t == TileType::Downstairs)
            else {
                break;
            };
            w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
            w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
            change_level(&mut w, true);
        }
    }
    for (line, n) in ["safe", "dangerous", "poor", "rich"].iter().zip(tally) {
        let share = n as f64 / floors as f64;
        assert!(
            (0.03..=0.35).contains(&share),
            "{line} spoke on {:.0}% of {floors} floors",
            share * 100.0
        );
    }
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

fn painted(w: &World, text: &str) -> Option<LogCategory> {
    w.resource::<GameLog>()
        .unread
        .iter()
        .find(|l| **l == *text)
        .map(|l| l.category)
}

#[test]
fn stairs_log_the_feeling_in_its_color() {
    let mut spoke = 0;
    for seed in 1..=40 {
        let (mut w, p) = fresh_run(seed);
        descend(&mut w, p);
        let f = level_feeling(&mut w);
        let lines = [
            (
                f.danger == Some(Danger::Safe),
                strings::feeling_safe(),
                Color::DarkMagenta,
            ),
            (
                f.danger == Some(Danger::Dangerous),
                strings::feeling_dangerous(),
                Color::Magenta,
            ),
            (
                f.riches == Some(Riches::Poor),
                strings::feeling_poor(),
                Color::DarkYellow,
            ),
            (
                f.riches == Some(Riches::Rich),
                strings::feeling_rich(),
                Color::Yellow,
            ),
        ];
        for (due, text, color) in lines {
            match (due, painted(&w, text)) {
                (true, got) => {
                    spoke += 1;
                    assert_eq!(got, Some(LogCategory::Room(color)), "seed {seed}: {text}");
                }
                (false, got) => assert_eq!(got, None, "seed {seed}: unexpected {text}"),
            }
        }
    }
    assert!(
        spoke > 0,
        "forty floors and the dungeon never had a feeling"
    );
}
