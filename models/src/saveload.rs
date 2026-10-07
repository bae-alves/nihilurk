//! The save file: one postcard blob, and the rules about what goes in it.
//!
//! There is exactly one save per run. It is written when the player quits,
//! destroyed when they die, and kept as "clear data" when they win (see
//! [`clear_data`]).
//!
//! ## Four things are deliberately left out
//!
//! Every one of them is cheaper to rebuild than to store, and leaving it out
//! is what keeps the file at roughly two kilobytes.
//!
//! 1. **The map.** A floor's layout is a pure function of `(seed, depth)`
//!    ([`crate::map::layout_rng`]), so [`regenerate_map`] rebuilds it exactly
//!    on load. Only the dark-room mask rides along, because a wand of light
//!    edits it.
//! 2. **The whole cosmetic layer** — bloodstains, corpse marks, lingering
//!    smoke, live particles, the screen shake, the scorekeeper's flash, and
//!    `FxRng` itself. None of it is gameplay, none of it is replayed, and
//!    none of it is worth a byte: the three map-sized overlays alone
//!    ([`BloodStains`], [`Corpses`], [`Smoke`]) would come to 2.2 KB — more
//!    than the entire save file they would be joining. [`load_game`] hands
//!    back a clean set of them, so a reloaded floor is the floor you left,
//!    scrubbed of the mess you made on it.
//! 3. **The message log.** A reload opens on a fresh welcome line rather than
//!    paying for a run's history in every write.
//! 4. **Anything a catalog row already says** — what a weapon does in flight,
//!    what a bow lends its wielder, what a ring grants. A row is the
//!    definition, so a save that stored these would only be storing the table
//!    twice; [`restore_from_catalog`] and [`RingDef::of`] read them back by
//!    name.
//!
//! ## Two things that must be
//!
//! * **Every serialised enum is written by variant position** and every
//!   struct by field order. Append variants and fields; never reorder one, or
//!   a saved trapdoor comes back as something else. (Appended *variants* keep
//!   their elders readable; appended *fields* do not — postcard is not
//!   self-describing and does not fill in absent fields.)
//! * **The format is versioned, and nothing migrates.** The file opens with
//!   [`SAVE_VERSION`], and a save under any other version is refused by name.
//!   `#[serde(default)]` marks the fields that joined late, but it rescues
//!   nothing: postcard is not self-describing, so a field added or removed
//!   changes every byte after it. Every field this file has ever gained has
//!   cost exactly that. Bump the constant with the change, and ship it as a
//!   minor release (`the_save_layout_only_changes_with_the_version` fails
//!   until you do, and `release/bump.lua` refuses a patch one); believe the
//!   attribute instead, and the next one ships as a change that quietly kills
//!   every run already in progress.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;

use crate::catalog::{RingDef, content_id_of, restore_from_catalog};
use crate::components::*;
use crate::constants::player::START_MAGIC;
use crate::effects::{
    ArmorBonus, ArmorDie, Grants, Held, Lifetime, PowerBonus, PowerDie, ThrowBonus, attach_effects,
    effects_of,
};
use crate::equipment::{Equipped, Slot};
use crate::map::{
    BloodStains, Corpses, Endless, FINAL_DEPTH, FxRng, GameRng, Map, RngSeed, Smoke, TileType,
    regenerate_map,
};
use crate::monsters::MonsterDef;
use crate::state::{Ending, GameState};
use rand_chacha::ChaCha12Rng;

/// The 16-colour terminal palette, packed to one byte instead of a debug string.
fn color_to_u8(c: &Color) -> u8 {
    match c {
        Color::Black => 0,
        Color::DarkGrey => 1,
        Color::Red => 2,
        Color::DarkRed => 3,
        Color::Green => 4,
        Color::DarkGreen => 5,
        Color::Yellow => 6,
        Color::DarkYellow => 7,
        Color::Blue => 8,
        Color::DarkBlue => 9,
        Color::Magenta => 10,
        Color::DarkMagenta => 11,
        Color::Cyan => 12,
        Color::DarkCyan => 13,
        Color::White => 14,
        Color::Grey => 15,
        _ => 14,
    }
}

fn u8_to_color(n: u8) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::DarkGrey,
        2 => Color::Red,
        3 => Color::DarkRed,
        4 => Color::Green,
        5 => Color::DarkGreen,
        6 => Color::Yellow,
        7 => Color::DarkYellow,
        8 => Color::Blue,
        9 => Color::DarkBlue,
        10 => Color::Magenta,
        11 => Color::DarkMagenta,
        12 => Color::Cyan,
        13 => Color::DarkCyan,
        15 => Color::Grey,
        _ => Color::White,
    }
}

/// One saved entity. Every component slot is an `Option`, which postcard encodes
/// as a single discriminant byte when empty — so absent components cost 1 byte
/// each with no field-name overhead. String fields borrow straight from the ECS
/// on save and from the file buffer on load.
/// How long a saved effect was being held for.
///
/// Deliberately **not** [`Lifetime`]: there is no `WhileEquipped` variant,
/// because a gear-lent effect is never written to a save — the gear is saved,
/// and lending it again on load is how it comes back. Leaving the variant out
/// means that rule is enforced by the type rather than remembered by a mask.
#[derive(Serialize, Deserialize, Clone, Copy)]
enum SavedLifetime {
    Permanent,
    Floor,
    Turns(u32),
    NextAction,
}

/// One held effect, as it goes to disk: the stable id off its [`EFFECTS`] row
/// and how long it had left.
#[derive(Serialize, Deserialize)]
struct SavedEffect {
    id: String,
    lifetime: SavedLifetime,
}

impl SavedEffect {
    /// `None` for a gear-lent effect, which is not saved.
    fn of(held: &Held) -> Self {
        let lifetime = match held.lifetime {
            Lifetime::Permanent => SavedLifetime::Permanent,
            Lifetime::Floor => SavedLifetime::Floor,
            Lifetime::Turns(n) => SavedLifetime::Turns(n),
            Lifetime::NextAction => SavedLifetime::NextAction,
            Lifetime::WhileEquipped(_) => SavedLifetime::Floor,
        };
        Self {
            id: held.id.to_string(),
            lifetime,
        }
    }

    /// Back to a runtime entry, resolving the id against this build's
    /// [`EFFECTS`]. `None` when the id is not one this build knows — a retired
    /// row, or a save from a newer build.
    fn held(&self) -> Option<Held> {
        let effect = crate::effects::Effect::by_id(&self.id)?;
        let lifetime = match self.lifetime {
            SavedLifetime::Permanent => Lifetime::Permanent,
            SavedLifetime::Floor => Lifetime::Floor,
            SavedLifetime::Turns(n) => Lifetime::Turns(n),
            SavedLifetime::NextAction => Lifetime::NextAction,
        };
        Some(Held {
            id: effect.id,
            lifetime,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct EntitySave<'a> {
    position: Option<(u16, u16)>,
    /// (glyph, palette index)
    renderable: Option<(char, u8)>,
    player: bool,
    hidden: bool,
    /// Intrinsically unseeable (phantom, stashed item).
    invisible: bool,
    consume: bool,
    /// Marker for the Element of Yoord.
    amulet: bool,
    #[serde(borrow)]
    name: Option<Cow<'a, str>>,
    /// (range, fog-of-war bitset). `visible_tiles` is never saved: the
    /// visibility system rebuilds it on the first frame after load.
    viewshed: Option<(u16, FixedBitSet)>,
    /// (hp, max_hp, armor, power, max_power, armor_bonus, power_bonus)
    fighter: Option<(i32, i32, i32, i32, i32, i32, i32)>,
    /// The player's magic-point pool.
    magic: Option<Magic>,
    faction: Option<Faction>,
    /// Indices into the saved entity list.
    backpack: Option<Vec<u32>>,
    score: Option<i64>,
    mob: Option<MovementType>,
    /// Marker only — the display name rides along in [`EntitySave::name`].
    item: bool,
    value: Option<i32>,
    potion: Option<PotionEffect>,
    battery: Option<i8>,
    wand: Option<WandEffect>,
    ranged: Option<i32>,
    scroll: Option<ScrollEffect>,
    ring: Option<RingEffect>,
    /// Which slot this piece of gear occupies, and (see
    /// [`EntitySave::equipped_by`]) who is wearing it.
    equipped: Option<Slot>,
    /// The combat modifiers this entity contributes: weapon class, armour class,
    /// and the flat bonuses an enchantment rolled onto them.
    power_die: Option<i32>,
    power_bonus: Option<i32>,
    armor_die: Option<i32>,
    armor_bonus: Option<i32>,
    /// A bow's plus. Every other modifier a launcher might carry is zero, so
    /// this is the only one worth a byte.
    throw_bonus: Option<i32>,
    /// How many of a stacking item this slot holds — a quiver of arrows.
    stack: Option<u8>,
    /// Marker: this equipment is cursed and can't be taken off once equipped.
    curse: bool,
    /// Marker: this equipment's enchantment plus and curse status are known —
    /// worn at least once, or singled out by a scroll of identify.
    known_quality: bool,
    /// A vorpalized weapon's `bane` species (scroll of vorpalize weapon).
    #[serde(borrow)]
    vorpal: Option<Cow<'a, str>>,
    /// The marker effects this entity owns in its own right — what it was born
    /// with, plus or minus whatever a wand of cancellation or a polymorph has
    /// done since, each with the lifetime it is being held for.
    ///
    /// Effects merely on loan from equipped gear are excluded, and
    /// [`SavedLifetime`] has no variant that could express one: load re-lends
    /// them to the wearer along with the gear itself.
    effects: Vec<SavedEffect>,
    /// A creature's movement tempo. The energy pool is transient and resets to 0.
    speed: Option<SpeedKind>,
    /// (effect, reveal style, already discovered) for a floor trap.
    trap: Option<(TrapEffect, TrapReveal, bool)>,
    /// A coin's effect and the dial it works with — see `Pickup`. Both are on
    /// the catalog row, but a coin is spent by the entity rather than looked up
    /// by name, so the entity carries them.
    #[serde(default)]
    pickup: Option<(PickupEffect, i32)>,
    /// The two promises a staircase settles (`Plated`, `Forged`). Unlike every
    /// other condition they are not lifted by the stairs, so a save that forgot
    /// them would quietly eat a coin.
    #[serde(default)]
    plated: bool,
    #[serde(default)]
    forged: bool,
    /// The player's active-spell bar. Nothing else in the game carries one yet.
    #[serde(default)]
    spellset: Option<Vec<SpellEffect>>,
    /// Who is wearing this piece of gear, as an index into the saved entity
    /// list — `None` when it is loose in a pack or on the floor. An index
    /// rather than an id because ids are ephemeral; it is remapped on load the
    /// same way [`EntitySave::backpack`] is.
    #[serde(default)]
    equipped_by: Option<u32>,
    /// Marker: the player's boon companion. Its [`Faction::Ally`] rides in
    /// [`EntitySave::faction`].
    #[serde(default)]
    helper: bool,
    /// Where an aggravated monster is heading, `(tx, ty)`. See [`Aggravated`].
    #[serde(default)]
    aggravated: Option<(u16, u16)>,
    /// The player's pull toward the cacodaemon/eudaemon poles. See
    /// [`Alignment`].
    #[serde(default)]
    alignment: Option<i8>,
    /// A deck's remaining cards, bottom first. See [`Deck`].
    #[serde(default)]
    deck: Option<Vec<Card>>,
    /// A rune's effect and whether it still holds a charge.
    #[serde(default)]
    rune: Option<(RuneEffect, bool)>,
    /// The id of the catalog row this entity was made from (see
    /// [`content_id_of`]). The loader reads the row back by it, and rebuilds
    /// [`Name`] from it, so nothing depends on what a row is called in the
    /// language that wrote the save. `None` for the player, whose name is
    /// whatever they typed, and for anything with no such row.
    #[serde(borrow)]
    content: Option<Cow<'a, str>>,
}

/// The version of the bytes [`save_game`] writes, stored as the file's first
/// field. Bump it whenever a saved struct or enum changes shape: a field added,
/// removed or reordered, a variant inserted anywhere but the end.
///
/// A file under any other version is refused by name instead of failing as a
/// postcard parse error. `release/bump.lua` reads this line: it refuses a
/// `patch` release when the value changed since the last tag, because a changed
/// save format is a minor bump.
pub const SAVE_VERSION: u16 = 1;

/// Splits a save file into its version and the [`SaveGame`] behind it, and
/// refuses any version but [`SAVE_VERSION`]. A file written before saves were
/// versioned starts with a different number, so it is refused here too.
fn decode(bytes: &[u8]) -> std::io::Result<SaveGame<'_>> {
    let invalid = |e: postcard::Error| std::io::Error::new(std::io::ErrorKind::InvalidData, e);
    let (version, rest) = postcard::take_from_bytes::<u16>(bytes).map_err(invalid)?;
    if version != SAVE_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "this save is version {version}, but this build reads version {SAVE_VERSION}: \
                 it was written by another release, or before saves were versioned"
            ),
        ));
    }
    postcard::from_bytes(rest).map_err(invalid)
}

#[derive(Serialize, Deserialize)]
struct SaveGame<'a> {
    #[serde(borrow)]
    entities: Vec<EntitySave<'a>>,
    #[serde(borrow)]
    player_name: Cow<'a, str>,
    /// The current dungeon depth.
    depth: u8,
    /// How many times the player has changed floors — salts `content_rng`, so a
    /// reload lands on the same re-roll of the current floor's contents.
    floor_changes: u32,
    /// The seed the run was originally created from.
    rng_seed: u64,
    /// The live RNG state, so the stream continues exactly where it left off.
    rng_state: ChaCha12Rng,
    /// The current floor's dark-room mask (see [`Map::dark`]). Rebuilt from the
    /// seed on load, then overwritten with this so any room a wand of light lit
    /// stays lit.
    dark_tiles: FixedBitSet,
    /// The current floor's cracked doorways (see [`Map::inert_doors`]),
    /// restored over the seed-built map the same way as `dark_tiles`.
    inert_doors: FixedBitSet,
    /// Tiles a wand of digging turned from rock to passage: the one thing the
    /// seed cannot rebuild about the map.
    #[serde(default)]
    dug_tiles: Vec<u32>,
    /// "Clear data": set when the run was won. The file is kept rather than
    /// deleted; the loader recognises it and asks before starting over.
    cleared: bool,
    /// Whether the spirits have turned on the player for good. See
    /// [`crate::components::SpiritsHostile`].
    #[serde(default)]
    spirits_hostile: bool,
    /// The bestiary row the player wears (`-am <species>`), by [`MonsterDef::name`],
    /// the id that survives a translation. Stored, not read back from the
    /// player's [`Name`]: a nihil who typed "dragon" at the prompt has the same
    /// name as a dragon.
    #[serde(borrow)]
    monster_body: Option<Cow<'a, str>>,
}

/// The winner's details, pulled from a won game's clear-data save file.
pub struct ClearData {
    /// The name the winner played under, already run through
    /// [`strip_control_chars`] so it is safe to print to a terminal.
    pub player_name: String,
}

/// Strips control characters (C0, DEL and C1) from a string read out of a save
/// file.
///
/// A save is untrusted the moment it can come from anywhere but this build's
/// own [`save_game`] — a shared file, a bug report attachment — and its
/// `player_name` reaches a real terminal verbatim (`clear_data_prompt`, the
/// status line) rather than through a bounds check like the entity indices
/// below. Without this, a crafted name carrying an escape sequence runs on
/// whoever loads the file.
///
/// ```
/// use models::strip_control_chars;
///
/// // The escape byte goes; the text around it stays.
/// assert_eq!(strip_control_chars("nihil\x1b[31m"), "nihil[31m");
/// ```
pub fn strip_control_chars(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// If `path` holds the clear data of a won run, returns the winner's name.
/// `Ok(None)` for an ordinary save — or one this build can no longer parse, so
/// the normal load path can report that instead.
pub fn clear_data(path: &str) -> std::io::Result<Option<ClearData>> {
    let bytes = std::fs::read(path)?;
    let Ok(save) = decode(&bytes) else {
        return Ok(None);
    };
    Ok(save.cleared.then(|| ClearData {
        player_name: strip_control_chars(&save.player_name),
    }))
}

/// Serializes the world to a compact postcard save file. The map is not saved:
/// it is rebuilt from the seed and the depth on load (see [`regenerate_map`]),
/// which is exact because a floor's layout depends on nothing else. Nor is the
/// message log — a reload starts with a blank one rather than paying for its
/// history in every save file.
///
/// The world is only read. Saving changes nothing about the run it describes —
/// not a monster's held weapon, not the player's worn armour — so a save
/// written mid-fight describes the fight still in progress.
///
/// The save struct borrows everything it can (names, item labels) straight out
/// of the ECS, so no second copy of the world is built in RAM, and the bytes
/// are streamed to disk through a `BufWriter` rather than buffered.
/// The tiles a wand of digging has opened on this floor, as indices into
/// [`Map::tiles`]: whatever the seed builds as rock that no longer is.
fn dug_tiles(world: &World) -> Vec<u32> {
    let pristine = crate::map::pristine_tiles(
        world,
        world.resource::<RngSeed>().0,
        world.resource::<Depth>().what,
    );
    world
        .resource::<Map>()
        .tiles
        .iter()
        .zip(pristine)
        .enumerate()
        .filter(|&(_, (&now, was))| was == TileType::Wall && now != TileType::Wall)
        .map(|(i, _)| i as u32)
        .collect()
}

/// Writes the whole run to `path` as a postcard file, replacing whatever was
/// there. Entities are saved by index, so [`load_game`] can rebuild every link
/// between them (a pack's items, who wears what).
///
/// Three things are left out on purpose. The map is rebuilt from the seed and
/// the depth, with only the tiles that changed (dark, inert doors, dug walls)
/// saved on top. The message log starts fresh on load. And [`FxRng`] and
/// [`Speed::energy`] reset: one only draws decoration, the other is a transient
/// that zero is a fair restart for.
///
/// Fails with the I/O error if `path` cannot be created or written, and wraps a
/// serialisation failure in [`std::io::ErrorKind::Other`]. A failed write can
/// leave a truncated file at `path`.
///
/// Panics if the world has no [`PlayerName`], [`Depth`], [`RngSeed`],
/// [`GameRng`] or [`Map`] resource. A world set up by the engine and
/// [`crate::initialize_world`] has them all.
///
/// ```
/// use bevy_ecs::prelude::*;
/// use models::*;
/// use rand::{RngCore, SeedableRng};
///
/// # fn fresh(seed: u64, name: &str) -> World {
/// #     let mut w = World::new();
/// #     w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
/// #     w.insert_resource(RngSeed(seed));
/// #     w.init_resource::<GameLog>();
/// #     w.insert_resource(PlayerName { what: name.into() });
/// #     w
/// # }
/// let path = std::env::temp_dir().join(format!("nihilurk-doc-{}.sav", std::process::id()));
/// let path = path.to_str().unwrap();
///
/// let mut run = fresh(1, "NIHIL");
/// initialize_world(&mut run);
/// save_game(&mut run, path).unwrap();
///
/// // A reload picks the dice up where the run left them, not at a fresh draw.
/// let mut reloaded = fresh(2, "OTHER");
/// load_game(&mut reloaded, path).unwrap();
/// let a = run.resource_mut::<GameRng>().0.next_u64();
/// let b = reloaded.resource_mut::<GameRng>().0.next_u64();
/// assert_eq!(a, b);
/// assert_eq!(reloaded.resource::<PlayerName>().what, "NIHIL");
/// # std::fs::remove_file(path).unwrap();
/// ```
pub fn save_game(world: &mut World, path: &str) -> std::io::Result<()> {
    crate::items::thaw_into_pack(world);
    let mut ents: Vec<Entity> = world.iter_entities().map(|e| e.id()).collect();
    ents.sort_by_key(|e| e.index());
    let index_map: HashMap<Entity, u32> = ents
        .iter()
        .enumerate()
        .map(|(i, e)| (*e, i as u32))
        .collect();

    let mut entities = Vec::with_capacity(ents.len());
    for &e in &ents {
        let er = world.entity(e);
        let name = er.get::<Name>().map(|n| n.what.as_str());
        entities.push(EntitySave {
            position: er.get::<Position>().map(|p| (p.x, p.y)),
            renderable: er
                .get::<Renderable>()
                .map(|r| (r.glyph, color_to_u8(&r.color))),
            player: er.contains::<Player>(),
            hidden: er.contains::<Hidden>(),
            invisible: er.contains::<Invisible>(),
            consume: er.contains::<Consume>(),
            amulet: er.contains::<Amulet>(),
            name: name.map(Cow::Borrowed),
            viewshed: er
                .get::<Viewshed>()
                .map(|v| (v.range, v.revealed_tiles.clone())),
            fighter: er.get::<Fighter>().map(|f| {
                (
                    f.hp,
                    f.max_hp,
                    f.armor,
                    f.power,
                    f.max_power,
                    f.armor_bonus,
                    f.power_bonus,
                )
            }),
            magic: er.get::<Magic>().copied(),
            faction: er.get::<Faction>().copied(),
            backpack: er.get::<Backpack>().map(|b| {
                b.items
                    .iter()
                    .filter_map(|i| index_map.get(i).copied())
                    .collect()
            }),
            score: er.get::<Score>().map(|s| s.value),
            mob: er.get::<Mob>().map(|m| m.movement_type),
            item: er.contains::<Item>(),
            value: er.get::<Value>().map(|v| v.amount),
            potion: er.get::<Potion>().map(|p| p.effect),
            battery: er.get::<Battery>().map(|b| b.charges),
            wand: er.get::<Wand>().map(|w| w.effect),
            ranged: er.get::<Ranged>().map(|r| r.range),
            scroll: er.get::<Scroll>().map(|s| s.effect),
            rune: er.get::<Rune>().map(|r| (r.effect, r.charged)),
            ring: er.get::<Ring>().map(|r| r.effect),
            equipped: er.get::<Equipped>().map(|e| e.slot),
            power_die: er.get::<PowerDie>().map(|m| m.0),
            power_bonus: er.get::<PowerBonus>().map(|m| m.0),
            armor_die: er.get::<ArmorDie>().map(|m| m.0),
            armor_bonus: er.get::<ArmorBonus>().map(|m| m.0),
            throw_bonus: er.get::<ThrowBonus>().map(|m| m.0),
            stack: er.get::<Stack>().map(|s| s.count),
            curse: er.contains::<Curse>(),
            known_quality: er.contains::<KnownQuality>(),
            vorpal: er.get::<Vorpal>().map(|v| Cow::Borrowed(v.bane.as_str())),
            effects: effects_of(world, e).iter().map(SavedEffect::of).collect(),
            speed: er.get::<Speed>().map(|s| s.kind),
            trap: er.get::<Trap>().map(|t| (t.effect, t.reveal, t.revealed)),
            pickup: er.get::<Pickup>().map(|p| (p.effect, p.amount)),
            plated: er.contains::<Plated>(),
            forged: er.contains::<Forged>(),
            spellset: er.get::<Spellset>().map(|m| m.slots.clone()),
            equipped_by: er
                .get::<Equipped>()
                .and_then(|e| e.by)
                .and_then(|w| index_map.get(&w).copied()),
            helper: er.contains::<Helper>(),
            aggravated: er.get::<Aggravated>().map(|a| (a.tx, a.ty)),
            alignment: er.get::<Alignment>().map(|a| a.0),
            deck: er.get::<Deck>().map(|d| d.cards.clone()),
            content: match er.contains::<Player>() {
                true => None,
                false => name.and_then(content_id_of).map(Cow::Borrowed),
            },
        });
    }

    let monster_body = ents.iter().find_map(|&e| {
        let er = world.entity(e);
        er.contains::<Player>()
            .then(|| {
                er.get::<crate::body::MonsterBody>()
                    .map(|b| Cow::Borrowed(b.0.name))
            })
            .flatten()
    });

    let save = SaveGame {
        entities,
        player_name: Cow::Borrowed(world.resource::<PlayerName>().what.as_str()),
        depth: world.resource::<Depth>().what,
        floor_changes: world.get_resource::<FloorChanges>().map_or(0, |c| c.count),
        rng_seed: world.resource::<RngSeed>().0,
        rng_state: world.resource::<GameRng>().0.clone(),
        dark_tiles: world.resource::<Map>().dark.clone(),
        inert_doors: world.resource::<Map>().inert_doors.clone(),
        dug_tiles: dug_tiles(world),
        cleared: world.get_resource::<Ending>().is_some_and(|e| e.player_won),
        spirits_hostile: world.get_resource::<SpiritsHostile>().is_some_and(|s| s.0),
        monster_body,
    };

    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let mut writer = postcard::to_io(&(SAVE_VERSION, &save), writer)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writer.flush()
}

/// Rebuilds the world from a postcard save file. Inserts GameState, GameLog,
/// PlayerName, Depth and FloorChanges resources; all other resources must
/// already be present.
///
/// The message log is not part of the save — a reload always starts with a
/// fresh [`GameLog`] (just the welcome line), same as a brand new run.
pub fn load_game(world: &mut World, path: &str) -> std::io::Result<()> {
    let bytes = std::fs::read(path)?;
    let save = decode(&bytes)?;
    let body_def = match save.monster_body.as_deref() {
        None => None,
        Some(id) => Some(MonsterDef::lookup(id).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("this save names a species, {id:?}, that this build does not know"),
            )
        })?),
    };

    world.insert_resource(GameState::new());
    world.insert_resource(BloodStains::new());
    world.insert_resource(Smoke::new());
    world.insert_resource(Corpses::new());
    world.init_resource::<crate::bones::Bones>();
    world.insert_resource(GameLog::default());
    world.insert_resource(PlayerName {
        what: strip_control_chars(&save.player_name),
    });
    world.insert_resource(Depth { what: save.depth });
    world.insert_resource(FloorChanges {
        count: save.floor_changes,
    });
    world.insert_resource(RngSeed(save.rng_seed));
    world.insert_resource(GameRng(save.rng_state));
    world.insert_resource(FxRng::new(save.rng_seed));
    world.insert_resource(DungeonLord::default());
    world.insert_resource(SpiritsHostile(save.spirits_hostile));

    if let Some(def) = body_def {
        world.insert_resource(crate::body::StartingBody(crate::body::Body::Monster(def)));
    }
    regenerate_map(world, save.rng_seed, save.depth);
    world.resource_mut::<Map>().dark = save.dark_tiles;
    world.resource_mut::<Map>().inert_doors = save.inert_doors;
    {
        let mut map = world.resource_mut::<Map>();
        for i in save.dug_tiles {
            if let Some(tile) = map.tiles.get_mut(i as usize)
                && *tile == TileType::Wall
            {
                *tile = TileType::Passage;
            }
        }
    }

    // The deepest floor has no down-stair: the seed-built map still carries one,
    // so carve it back to plain floor. The Element of Yoord entity (or its place
    // in the pack) comes back from the save itself.
    let endless = world.get_resource::<Endless>().is_some_and(|e| e.enabled);
    if save.depth >= FINAL_DEPTH && !endless {
        let mut map = world.resource_mut::<Map>();
        if let Some(i) = map.tiles.iter().position(|&t| t == TileType::Downstairs) {
            map.tiles[i] = TileType::Room;
        }
    }

    let count = save.entities.len();

    // `backpack` and `equipped_by` are raw indices into this same list, with
    // nothing upstream bounding them — a save is untrusted the moment it can
    // come from anywhere but this build's own `save_game` (a shared file, a
    // bug report attachment). Check every one before the loop below indexes
    // `new_ents` with it, so a crafted save fails to load instead of panicking.
    let index_in_range = |idx: u32| (idx as usize) < count;
    let indices_valid = save.entities.iter().all(|es| {
        es.backpack
            .as_ref()
            .is_none_or(|items| items.iter().all(|&idx| index_in_range(idx)))
            && es.equipped_by.is_none_or(index_in_range)
    });
    if !indices_valid {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "save file names an entity index outside its own entity list",
        ));
    }

    let mut new_ents = Vec::with_capacity(count);
    for _ in 0..count {
        new_ents.push(world.spawn_empty().id());
    }

    let mut retired = 0usize;
    let mut bearers: Vec<Entity> = Vec::new();

    for (i, es) in save.entities.into_iter().enumerate() {
        let mut em = world.entity_mut(new_ents[i]);

        if let Some((x, y)) = es.position {
            em.insert(Position { x, y });
        }
        if let Some((glyph, color)) = es.renderable {
            em.insert(Renderable {
                glyph,
                color: u8_to_color(color),
            });
        }
        if es.player {
            em.insert(Player);
        }
        if es.hidden {
            em.insert(Hidden);
        }
        if es.invisible {
            em.insert(Invisible);
        }
        if es.consume {
            em.insert(Consume);
        }
        if es.amulet {
            em.insert(Amulet);
        }
        let content = es.content.as_deref();
        let entity_name = match content {
            Some(id) => Some(strings::content_name(id).to_string()),
            None => es.name.map(|n| n.into_owned()),
        };
        if let Some(n) = &entity_name {
            em.insert(Name { what: n.clone() });
        }
        if let Some((range, revealed_tiles)) = es.viewshed {
            em.insert(Viewshed {
                visible_tiles: Vec::new(),
                revealed_tiles,
                range,
                dirty: true,
            });
        }
        if let Some((hp, max_hp, armor, power, max_power, armor_bonus, power_bonus)) = es.fighter {
            em.insert(Fighter {
                hp,
                max_hp,
                armor,
                power,
                max_power,
                armor_bonus,
                power_bonus,
            });
        }
        // The player always carries a magic pool; fall back to a full one if the
        // save somehow lacks it.
        if let Some(magic) = es.magic.or_else(|| {
            es.player.then_some(Magic {
                points: START_MAGIC,
                max_points: START_MAGIC,
            })
        }) {
            em.insert(magic);
        }
        if let Some(f) = es.faction {
            em.insert(f);
        }
        if let Some(items) = es.backpack {
            let mapped: Vec<Entity> = items.iter().map(|&idx| new_ents[idx as usize]).collect();
            em.insert(Backpack { items: mapped });
        }
        if let Some(s) = es.score {
            em.insert(Score { value: s });
        }
        if let Some(m) = es.mob {
            let movement_type = match m {
                MovementType::Aggravated { tx, ty } => {
                    em.insert(Aggravated { tx, ty });
                    MovementType::Chase
                }
                other => other,
            };
            em.insert(Mob { movement_type });
        }
        if let Some((tx, ty)) = es.aggravated {
            em.insert(Aggravated { tx, ty });
        }
        if es.player || es.mob.is_some() {
            em.insert(Blood);
        }
        if es.item {
            em.insert(Item);
        }
        if let Some(v) = es.value {
            em.insert(Value { amount: v });
        }
        if let Some(p) = es.potion {
            em.insert(Potion { effect: p });
        }
        if let Some(b) = es.battery {
            em.insert(Battery { charges: b });
        }
        if let Some(w) = es.wand {
            em.insert(Wand { effect: w });
        }
        if let Some(r) = es.ranged {
            em.insert(Ranged { range: r });
        }
        if let Some(effect) = es.scroll {
            em.insert(Scroll { effect });
        }
        if let Some((effect, charged)) = es.rune {
            em.insert(Rune { effect, charged });
        }
        if let Some(effect) = es.ring {
            em.insert(Ring { effect });
            let def = RingDef::of(effect);
            em.insert(Grants(def.grants));
            if let Some(on_wear) = def.on_wear {
                em.insert(on_wear);
            }
        }
        if let Some(slot) = es.equipped {
            // The wearer comes back as an index into the saved entity list and
            // is remapped here, the same way pack contents are: gear stays on
            // across a save, in the same hand and on the same body.
            let by = es.equipped_by.map(|i| new_ents[i as usize]);
            if let Some(wearer) = by {
                if !bearers.contains(&wearer) {
                    bearers.push(wearer);
                }
            }
            em.insert(Equipped { by, slot });
        }
        if let Some(n) = es.power_die {
            em.insert(PowerDie(n));
        }
        if let Some(id) = content {
            restore_from_catalog(&mut em, id);
        }
        if let Some(n) = es.power_bonus {
            em.insert(PowerBonus(n));
        }
        if let Some(n) = es.armor_die {
            em.insert(ArmorDie(n));
        }
        if let Some(n) = es.armor_bonus {
            em.insert(ArmorBonus(n));
        }
        if let Some(n) = es.throw_bonus {
            em.insert(ThrowBonus(n));
        }
        if let Some(count) = es.stack {
            em.insert(Stack { count });
        }
        if es.curse {
            em.insert(Curse);
        }
        if es.known_quality {
            em.insert(KnownQuality);
        }
        if let Some(bane) = es.vorpal {
            em.insert(Vorpal {
                bane: bane.into_owned(),
            });
        }
        let species = match es.player {
            true => body_def,
            false => content.and_then(MonsterDef::lookup),
        };
        if let Some(def) = species {
            if !def.grants.is_empty() {
                em.insert(Grants(def.grants));
            }
            if let Some(kind) = def.spirit_kind {
                em.insert(kind);
            }
            if let Some(event) = def.spirit_event {
                em.insert(event);
            }
            if es.player {
                em.insert(crate::body::MonsterBody(def));
            }
        }
        let held: Vec<Held> = es.effects.iter().filter_map(SavedEffect::held).collect();
        retired += es.effects.len() - held.len();
        attach_effects(&mut em, &held);
        if es.player || es.mob.is_some() {
            em.insert(Speed::new(es.speed.unwrap_or_default()));
        }
        if let Some((effect, reveal, revealed)) = es.trap {
            em.insert(Trap {
                effect,
                reveal,
                revealed,
            });
        }
        if let Some((effect, amount)) = es.pickup {
            em.insert(Pickup { effect, amount });
        }
        if es.plated {
            em.insert(Plated);
        }
        if es.forged {
            em.insert(Forged);
        }
        if es.helper {
            em.insert(Helper);
        }
        if let Some(slots) = es.spellset {
            em.insert(Spellset { slots });
        }
        if let Some(a) = es.alignment {
            em.insert(Alignment(a));
        }
        if let Some(cards) = es.deck {
            em.insert(Deck { cards });
        }
    }

    for bearer in bearers {
        crate::equipment::sync_equipment_effects(world, bearer);
    }

    if retired > 0 {
        let line = match retired {
            1 => strings::retired_enchantment_singular().to_string(),
            n => strings::retired_enchantment_plural(n),
        };
        world.resource_mut::<GameLog>().add(line);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// An [`EntitySave`] with every field at its empty default, so a test only
    /// has to name the field it actually cares about.
    fn blank_entity<'a>() -> EntitySave<'a> {
        EntitySave {
            position: None,
            renderable: None,
            player: false,
            hidden: false,
            invisible: false,
            consume: false,
            amulet: false,
            name: None,
            viewshed: None,
            fighter: None,
            magic: None,
            faction: None,
            backpack: None,
            score: None,
            mob: None,
            item: false,
            value: None,
            potion: None,
            battery: None,
            wand: None,
            ranged: None,
            scroll: None,
            rune: None,
            ring: None,
            equipped: None,
            power_die: None,
            power_bonus: None,
            armor_die: None,
            armor_bonus: None,
            throw_bonus: None,
            stack: None,
            curse: false,
            known_quality: false,
            vorpal: None,
            effects: Vec::new(),
            speed: None,
            trap: None,
            pickup: None,
            plated: false,
            forged: false,
            spellset: None,
            equipped_by: None,
            helper: false,
            aggravated: None,
            alignment: None,
            deck: None,
            content: None,
        }
    }

    /// A [`SaveGame`] with every field at its empty default.
    fn blank_save<'a>(entities: Vec<EntitySave<'a>>) -> SaveGame<'a> {
        SaveGame {
            entities,
            player_name: Cow::Borrowed("X"),
            depth: 1,
            floor_changes: 0,
            rng_seed: 1,
            rng_state: ChaCha12Rng::seed_from_u64(1),
            dark_tiles: FixedBitSet::with_capacity(1),
            inert_doors: FixedBitSet::with_capacity(1),
            dug_tiles: Vec::new(),
            cleared: false,
            spirits_hostile: false,
            monster_body: None,
        }
    }

    /// The bytes of `save` as `save_game` would write them, under `version`.
    fn encode(version: u16, save: &SaveGame) -> Vec<u8> {
        postcard::to_allocvec(&(version, save)).unwrap()
    }

    /// A save is untrusted the moment it can come from anywhere but this
    /// build's own [`save_game`] — a shared file, a bug report attachment.
    /// `backpack` and `equipped_by` are raw indices into the saved entity
    /// list with nothing upstream bounding them, so a crafted save naming an
    /// index past the end of the list must fail to load rather than index
    /// straight into `new_ents`.
    fn crafted_save_with_backpack_index(index: u32) -> Vec<u8> {
        let mut entity = blank_entity();
        entity.backpack = Some(vec![index]);
        encode(SAVE_VERSION, &blank_save(vec![entity]))
    }

    fn load_bytes(tag: &str, bytes: &[u8]) -> std::io::Result<World> {
        let path = std::env::temp_dir().join(format!(
            "nihilurk-saveload-unit-{}-{tag}.sav",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        let mut world = World::new();
        world.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2)));
        world.insert_resource(RngSeed(2));
        world.init_resource::<GameLog>();
        world.insert_resource(PlayerName { what: "Y".into() });
        let result = load_game(&mut world, path.to_str().unwrap());
        let _ = std::fs::remove_file(&path);
        result.map(|_| world)
    }

    #[test]
    fn an_out_of_range_backpack_index_is_rejected_not_indexed() {
        let bytes = crafted_save_with_backpack_index(1);
        assert!(
            load_bytes("oob", &bytes).is_err(),
            "a crafted save with an out-of-range backpack index must error, not panic"
        );
    }

    #[test]
    fn a_save_from_another_version_is_refused_and_says_which() {
        let other = SAVE_VERSION + 1;
        let bytes = encode(other, &blank_save(vec![blank_entity()]));
        let err = load_bytes("version", &bytes)
            .err()
            .expect("a save under another version must not load");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        let said = err.to_string();
        assert!(
            said.contains(&other.to_string()) && said.contains(&SAVE_VERSION.to_string()),
            "the refusal should name both versions: {said}"
        );
    }

    #[test]
    fn clear_data_leaves_a_save_from_another_version_to_the_loader() {
        let mut save = blank_save(vec![blank_entity()]);
        save.cleared = true;
        let path = std::env::temp_dir().join(format!(
            "nihilurk-saveload-unit-{}-clear-version.sav",
            std::process::id()
        ));
        std::fs::write(&path, encode(SAVE_VERSION + 1, &save)).unwrap();
        let clear = clear_data(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(clear.is_none());
    }

    #[test]
    fn a_save_naming_a_species_this_build_lacks_is_refused() {
        let mut save = blank_save(vec![blank_entity()]);
        save.monster_body = Some(Cow::Borrowed("no such species"));
        let err = load_bytes("species", &encode(SAVE_VERSION, &save))
            .err()
            .expect("an unknown species must not load as a plain nihil");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("no such species"));
    }

    /// Every saved field set to a value no empty default shares, the wide ones
    /// past a one-byte varint, and each enum at its last variant: inserting a
    /// variant anywhere before it moves its index and changes the bytes, while
    /// appending one (which the module docs allow) does not. A new field is a
    /// compile error here, which is the point.
    fn full_entity<'a>() -> EntitySave<'a> {
        EntitySave {
            position: Some((700, 300)),
            renderable: Some(('@', 14)),
            player: true,
            hidden: true,
            invisible: true,
            consume: true,
            amulet: true,
            name: Some(Cow::Borrowed("name")),
            viewshed: Some((9, bits(&[3, 69]))),
            fighter: Some((300, 301, -2, 5, 6, 7, 8)),
            magic: Some(Magic {
                points: 3,
                max_points: 4,
            }),
            faction: Some(Faction::Spirits),
            backpack: Some(vec![1, 300]),
            score: Some(-5000),
            mob: Some(MovementType::Ambush),
            item: true,
            value: Some(300),
            potion: Some(PotionEffect::Polymorph),
            battery: Some(-3),
            wand: Some(WandEffect::Swapping),
            ranged: Some(300),
            scroll: Some(ScrollEffect::Atonement),
            ring: Some(RingEffect::Polymorph),
            equipped: Some(Slot::Finger),
            power_die: Some(300),
            power_bonus: Some(-300),
            armor_die: Some(301),
            armor_bonus: Some(-301),
            throw_bonus: Some(302),
            stack: Some(200),
            curse: true,
            known_quality: true,
            vorpal: Some(Cow::Borrowed("bane")),
            effects: vec![
                SavedEffect {
                    id: "first".into(),
                    lifetime: SavedLifetime::Turns(300),
                },
                SavedEffect {
                    id: "second".into(),
                    lifetime: SavedLifetime::NextAction,
                },
            ],
            speed: Some(SpeedKind::Quick),
            trap: Some((TrapEffect::Dart, TrapReveal::Triggered, true)),
            pickup: Some((PickupEffect::LearnRandomSpell, 300)),
            plated: true,
            forged: true,
            spellset: Some(vec![SpellEffect::GateDown]),
            equipped_by: Some(300),
            helper: true,
            aggravated: Some((700, 701)),
            alignment: Some(-100),
            deck: Some(vec![Card {
                face: CardFace::GoldenWind,
                reversed: true,
            }]),
            rune: Some((RuneEffect::Protection, true)),
            content: Some(Cow::Borrowed("arrow")),
        }
    }

    fn bits(set: &[usize]) -> FixedBitSet {
        let mut bits = FixedBitSet::with_capacity(70);
        bits.extend(set.iter().copied());
        bits
    }

    fn full_save<'a>() -> SaveGame<'a> {
        let mut save = blank_save(vec![full_entity(), blank_entity()]);
        save.player_name = Cow::Borrowed("player");
        save.depth = 200;
        save.floor_changes = 70_000;
        save.rng_seed = 0x0123_4567_89ab_cdef;
        save.dark_tiles = bits(&[0, 64]);
        save.inert_doors = bits(&[69]);
        save.dug_tiles = vec![1, 70_000];
        save.cleared = true;
        save.spirits_hostile = true;
        save.monster_body = Some(Cow::Borrowed("dragon"));
        save
    }

    fn split_header(bytes: &[u8]) -> (u16, &[u8]) {
        postcard::take_from_bytes::<u16>(bytes).expect("a version leads the file")
    }

    const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/save.bin");

    /// The bytes of [`full_save`] are checked in. A change to a saved struct,
    /// or to a serialised dependency such as the RNG, changes them, and the
    /// test then asks for two things in order: bump [`SAVE_VERSION`], then
    /// regenerate the file. It will not regenerate under the old version, so a
    /// format change cannot slip into a release as a patch.
    #[test]
    fn the_save_layout_only_changes_with_the_version() {
        let now = encode(SAVE_VERSION, &full_save());
        let (now_version, now_body) = split_header(&now);
        let golden = std::fs::read(GOLDEN).unwrap_or_default();
        let (golden_version, golden_body) = match golden.is_empty() {
            true => (0, &golden[..]),
            false => split_header(&golden),
        };
        if now_body != golden_body {
            assert_ne!(
                now_version, golden_version,
                "the saved layout changed, but SAVE_VERSION is still {now_version}. Bump it in \
                 saveload.rs: old saves no longer load, so this ships as a minor release."
            );
        }
        if std::env::var_os("NIHILURK_REGEN_GOLDEN").is_some() {
            std::fs::create_dir_all(std::path::Path::new(GOLDEN).parent().unwrap()).unwrap();
            std::fs::write(GOLDEN, &now).unwrap();
            return;
        }
        assert!(
            now == golden,
            "tests/golden/save.bin is out of date. Regenerate it with \
             `NIHILURK_REGEN_GOLDEN=1 cargo test -p nihilurk-models --lib the_save_layout`"
        );
        decode(&golden).expect("the checked-in save still decodes");
    }

    fn load_one(tag: &str, entity: EntitySave) -> World {
        load_bytes(tag, &encode(SAVE_VERSION, &blank_save(vec![entity])))
            .expect("a well-formed save loads")
    }

    #[test]
    fn a_saved_content_id_wins_over_the_saved_name() {
        let mut entity = blank_entity();
        entity.name = Some(Cow::Borrowed("a name no row has"));
        entity.content = Some(Cow::Borrowed("arrow"));
        let mut w = load_one("id-wins", entity);
        let arrow = w.query_filtered::<Entity, With<Projectile>>().single(&w);
        assert_eq!(
            crate::helpers::item_label(&w, arrow),
            strings::content_name("arrow")
        );
    }

    #[test]
    fn a_saved_name_alone_restores_nothing_from_the_catalog() {
        let mut entity = blank_entity();
        entity.name = Some(Cow::Borrowed("arrow"));
        let mut w = load_one("name-alone", entity);
        assert_eq!(w.query::<&Projectile>().iter(&w).count(), 0);
        let only = w.query::<Entity>().single(&w);
        assert_eq!(crate::helpers::item_label(&w, only), "arrow");
    }

    #[test]
    fn an_in_range_backpack_index_still_loads() {
        let bytes = crafted_save_with_backpack_index(0);
        assert!(load_bytes("ok", &bytes).is_ok());
    }

    /// A save carrying a `player_name` with control/escape bytes -- a shared
    /// save is untrusted the same way a crafted backpack index is, and this
    /// field reaches a real terminal verbatim (`clear_data_prompt`, the HUD)
    /// rather than an index bounds-check.
    fn crafted_save_with_player_name(name: &str, cleared: bool) -> Vec<u8> {
        let mut save = blank_save(vec![blank_entity()]);
        save.player_name = Cow::Borrowed(name);
        save.cleared = cleared;
        encode(SAVE_VERSION, &save)
    }

    #[test]
    fn a_loaded_player_name_has_its_control_bytes_stripped() {
        let evil = "Bae\u{1b}]0;pwned\u{7}";
        let bytes = crafted_save_with_player_name(evil, false);
        let world = load_bytes("ctrl-name", &bytes).expect("a bad name must not fail the load");
        let loaded = &world.resource::<PlayerName>().what;
        assert!(
            !loaded.chars().any(|c| c.is_control()),
            "loaded player name still has control bytes: {loaded:?}"
        );
        assert_eq!(loaded, "Bae]0;pwned");
    }

    #[test]
    fn clear_data_strips_control_bytes_from_the_winners_name() {
        let evil = "Bae\u{1b}]0;pwned\u{7}";
        let bytes = crafted_save_with_player_name(evil, true);
        let path = std::env::temp_dir().join(format!(
            "nihilurk-saveload-unit-{}-clear-ctrl.sav",
            std::process::id()
        ));
        std::fs::write(&path, &bytes).unwrap();
        let clear = clear_data(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        let name = clear
            .expect("cleared=true must read back as clear data")
            .player_name;
        assert!(
            !name.chars().any(|c| c.is_control()),
            "clear-data player name still has control bytes: {name:?}"
        );
    }
}
