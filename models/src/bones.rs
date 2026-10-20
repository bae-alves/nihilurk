//! The bones file: what a dead run leaves behind for the next one to find.
//!
//! One flat postcard file per depth (`path_for`), in [`Bones::dir`] next to
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
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::components::{Backpack, Depth, Name, Player, PlayerName, Stack};
use crate::effects::{ArmorBonus, MaxHpBonus, PowerBonus, ThrowBonus};
use crate::equipment::{Equipped, Slot, equipped_items};

/// Whether bones are saved and loaded at all this run — `false` under
/// `-nobones`. Checked by both [`deposit`] and
/// `crate::map::levels::spawn_bones_ghost` before either side of the file
/// ever gets touched.
#[derive(Resource)]
pub struct Bones {
    /// `true` unless the run started with `-nobones`.
    pub enabled: bool,
    /// Where the bones files live: [`crate::data_dir`], unless a test points
    /// it somewhere of its own.
    pub dir: PathBuf,
}

impl Default for Bones {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: crate::data_dir(),
        }
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
    max_hp_bonus: i32,
    stack: Option<u8>,
}

/// One dead character's leavings on one floor: who they were and everything
/// they carried. [`deposit`] writes it when the run ends, and [`take`] reads it
/// back, once, for a later character.
#[derive(Serialize, Deserialize)]
pub struct BonesFile {
    /// The dead character's name. The ghost that comes back for their gear is
    /// called by it.
    pub name: String,
    items: Vec<BonesItem>,
}

impl BonesFile {
    /// The saved items, as `(catalog name, worn slot, power+, armor+, throw+,
    /// max hp+, stack)` — everything `crate::map::levels::spawn_bones_ghost` needs and
    /// nothing it has to reach back into this module's private struct for.
    pub fn items(
        &self,
    ) -> impl Iterator<Item = (&str, Option<Slot>, i32, i32, i32, i32, Option<u8>)> {
        self.items.iter().map(|i| {
            (
                i.name.as_str(),
                i.equipped_slot,
                i.power_bonus,
                i.armor_bonus,
                i.throw_bonus,
                i.max_hp_bonus,
                i.stack,
            )
        })
    }
}

fn path_for(dir: &Path, depth: u8) -> PathBuf {
    dir.join(format!("bones-{depth}.sav"))
}

/// Writes the current floor's bones file from the player's own gear —
/// everything in [`Backpack`] plus everything [`equipped_items`] reports —
/// unless `-nobones` turned the mechanic off. Best-effort, like
/// [`crate::leaderboard::record`]: a run ending on a read-only directory
/// still gets its death screen.
pub fn deposit(world: &mut World) -> std::io::Result<()> {
    let Some(dir) = world
        .get_resource::<Bones>()
        .filter(|b| b.enabled)
        .map(|b| b.dir.clone())
    else {
        return Ok(());
    };
    let depth = world.resource::<Depth>().what;
    deposit_to(world, path_for(&dir, depth))
}

fn deposit_to(world: &mut World, path: impl AsRef<Path>) -> std::io::Result<()> {
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
                max_hp_bonus: world.get::<MaxHpBonus>(item).map_or(0, |m| m.0),
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
    crate::saveload::write_atomically(path, |w| w.write_all(&bytes))
}

/// The bones file for `depth` in `dir`, if one is waiting there — and gone
/// from disk the instant this returns, one encounter per death.
pub fn take(dir: &Path, depth: u8) -> Option<BonesFile> {
    take_from(path_for(dir, depth))
}

fn take_from(path: impl AsRef<Path>) -> Option<BonesFile> {
    let bytes = std::fs::read(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let mut bones: BonesFile = postcard::from_bytes(&bytes).ok()?;
    bones.name = crate::clean_name(&bones.name);
    Some(bones)
}

#[cfg(test)]
mod tests;
