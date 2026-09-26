//! The bones file: what a dead run leaves behind for the next one to find.
//!
//! One flat postcard file per depth ([`path_for`]), sitting next to
//! `leaderboard.sav` — global across seeds, and a plain file a player can
//! `rm` by hand if they'd rather not meet it. [`deposit`] writes one the
//! moment a character dies; [`take`] reads and deletes it the moment a later
//! character reaches that same depth on the way back out with the Element of
//! Yoord (see [`crate::map::holding_element_of_yoord`] and
//! `crate::map::levels::spawn_bones_ghost`, which is the only caller).
//!
//! Only equipped and backpacked items are captured — what the GDD calls
//! "past run loot" — each as a bare catalog name plus whatever enchantment
//! numbers `spawn_named` wouldn't otherwise know to give it back. This is
//! deliberately smaller than a save file's [`crate::components::Renderable`]-and-all
//! entity snapshot: the ghost that carries it back is a fresh spawn, not a
//! resurrection.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Backpack, Depth, Name, Player, PlayerName, Stack};
use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
use crate::equipment::{Equipped, Slot, equipped_items};

/// Whether bones are saved and loaded at all this run — `false` under
/// `-nobones`. Checked by both [`deposit`] and
/// `crate::map::levels::spawn_bones_ghost` before either side of the file
/// ever gets touched.
#[derive(Resource)]
pub struct Bones {
    pub enabled: bool,
}

impl Default for Bones {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// One item a dead character had on them — its catalog name (for
/// [`crate::spawn::spawn_named`]) and the numbers that name alone wouldn't
/// bring back.
#[derive(Serialize, Deserialize)]
struct BonesItem {
    name: String,
    /// `None` for a loose item; the slot it was worn in otherwise. Every
    /// `Some` here is force-cursed the moment it's spawned back, whatever it
    /// was cursed with at death.
    equipped_slot: Option<Slot>,
    power_bonus: i32,
    armor_bonus: i32,
    throw_bonus: i32,
    stack: Option<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct BonesFile {
    pub name: String,
    items: Vec<BonesItem>,
}

impl BonesFile {
    /// The saved items, as `(catalog name, worn slot, power+, armor+, throw+,
    /// stack)` — everything `crate::map::levels::spawn_bones_ghost` needs and
    /// nothing it has to reach back into this module's private struct for.
    pub fn items(&self) -> impl Iterator<Item = (&str, Option<Slot>, i32, i32, i32, Option<u8>)> {
        self.items.iter().map(|i| {
            (
                i.name.as_str(),
                i.equipped_slot,
                i.power_bonus,
                i.armor_bonus,
                i.throw_bonus,
                i.stack,
            )
        })
    }
}

fn path_for(depth: u8) -> String {
    format!("bones-{depth}.sav")
}

/// Writes the current floor's bones file from the player's own gear —
/// everything in [`Backpack`] plus everything [`equipped_items`] reports —
/// unless `-nobones` turned the mechanic off. Best-effort, like
/// [`crate::leaderboard::record`]: a run ending on a read-only directory
/// still gets its death screen.
pub fn deposit(world: &mut World) -> std::io::Result<()> {
    if !world.get_resource::<Bones>().is_some_and(|b| b.enabled) {
        return Ok(());
    }
    let depth = world.resource::<Depth>().what;
    deposit_to(world, &path_for(depth))
}

fn deposit_to(world: &mut World, path: &str) -> std::io::Result<()> {
    let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    else {
        return Ok(());
    };

    let mut carried = equipped_items(world, player);
    if let Some(bp) = world.get::<Backpack>(player) {
        carried.extend(bp.items.iter().copied());
    }

    let items: Vec<BonesItem> = carried
        .into_iter()
        .filter_map(|item| {
            let name = world.get::<Name>(item)?.what.clone();
            Some(BonesItem {
                name,
                equipped_slot: world.get::<Equipped>(item).map(|e| e.slot),
                power_bonus: world.get::<PowerBonus>(item).map_or(0, |m| m.0),
                armor_bonus: world.get::<ArmorBonus>(item).map_or(0, |m| m.0),
                throw_bonus: world.get::<ThrowBonus>(item).map_or(0, |m| m.0),
                stack: world.get::<Stack>(item).map(|s| s.count),
            })
        })
        .collect();

    let bones = BonesFile {
        name: world.resource::<PlayerName>().what.clone(),
        items,
    };
    let bytes = postcard::to_allocvec(&bones)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, bytes)
}

/// The bones file for `depth`, if one is waiting there — and gone from disk
/// the instant this returns, one encounter per death.
pub fn take(depth: u8) -> Option<BonesFile> {
    take_from(&path_for(depth))
}

fn take_from(path: &str) -> Option<BonesFile> {
    let bytes = std::fs::read(path).ok()?;
    let _ = std::fs::remove_file(path);
    postcard::from_bytes(&bytes).ok()
}

#[cfg(test)]
mod tests {
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
                .any(|&(name, slot, power, _, _, _)| name == "long sword"
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
        let _ = sword; // kept alive only to be captured by deposit_to above
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
        // `deposit`'s flag check is the one path that has to go through the
        // real depth-keyed file (`path_for`), not `deposit_to`'s temp path —
        // an out-of-range depth keeps it from colliding with an actual run's
        // `bones-N.sav` in this directory.
        let depth = 255;
        let path = path_for(depth);
        let _ = std::fs::remove_file(&path);
        let (mut w, player) = world_with_player();
        w.insert_resource(Bones { enabled: false });
        w.insert_resource(Depth { what: depth });

        let item = w
            .spawn(Name {
                what: "dagger".into(),
            })
            .id();
        w.get_mut::<Backpack>(player).unwrap().items.push(item);

        deposit(&mut w).unwrap();

        let result = take(depth);
        let _ = std::fs::remove_file(&path);
        assert!(
            result.is_none(),
            "the flag being off means deposit() itself never wrote a file"
        );
    }
}
