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
//! * **The format is not versioned.** `#[serde(default)]` marks the fields
//!   that joined late, but it rescues nothing: postcard is not
//!   self-describing, and a save written by an older build fails to parse.
//!   Every field this file has ever gained has cost exactly that — believe
//!   the attribute instead, and the next one ships as a change that quietly
//!   kills every run already in progress.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;

use crate::catalog::{RingDef, restore_from_catalog};
use crate::components::*;
use crate::constants::player::START_MAGIC;
use crate::effects::{
    ArmorBonus, ArmorDie, Grants, Held, Lifetime, PowerBonus, PowerDie, ThrowBonus, attach_effects,
    effects_of,
};
use crate::equipment::{Equipped, Slot};
use crate::map::{
    BloodStains, Corpses, FINAL_DEPTH, FxRng, GameRng, Map, RngSeed, Smoke, TileType,
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
            // `effects_of` filters these out before this is reached; mapping
            // it to `Permanent` would quietly make a borrowed ring permanent.
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
    /// "Clear data": set when the run was won. The file is kept rather than
    /// deleted; the loader recognises it and asks before starting over.
    cleared: bool,
}

/// The winner's details, pulled from a won game's clear-data save file.
pub struct ClearData {
    pub player_name: String,
}

/// If `path` holds the clear data of a won run, returns the winner's name.
/// `Ok(None)` for an ordinary save — or one this build can no longer parse, so
/// the normal load path can report that instead.
pub fn clear_data(path: &str) -> std::io::Result<Option<ClearData>> {
    let bytes = std::fs::read(path)?;
    let Ok(save) = postcard::from_bytes::<SaveGame>(&bytes) else {
        return Ok(None);
    };
    Ok(save.cleared.then(|| ClearData {
        player_name: save.player_name.into_owned(),
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
pub fn save_game(world: &mut World, path: &str) -> std::io::Result<()> {
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
            name: er.get::<Name>().map(|n| Cow::Borrowed(n.what.as_str())),
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
        });
    }

    let save = SaveGame {
        entities,
        player_name: Cow::Borrowed(world.resource::<PlayerName>().what.as_str()),
        depth: world.resource::<Depth>().what,
        floor_changes: world.get_resource::<FloorChanges>().map_or(0, |c| c.count),
        rng_seed: world.resource::<RngSeed>().0,
        rng_state: world.resource::<GameRng>().0.clone(),
        dark_tiles: world.resource::<Map>().dark.clone(),
        cleared: world.get_resource::<Ending>().is_some_and(|e| e.player_won),
    };

    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let mut writer = postcard::to_io(&save, writer)
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
    let save: SaveGame = postcard::from_bytes(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    world.insert_resource(GameState::new());
    world.insert_resource(BloodStains::new());
    world.insert_resource(Smoke::new());
    world.insert_resource(Corpses::new());
    world.insert_resource(GameLog::default());
    world.insert_resource(PlayerName {
        what: save.player_name.into_owned(),
    });
    world.insert_resource(Depth { what: save.depth });
    world.insert_resource(FloorChanges {
        count: save.floor_changes,
    });
    world.insert_resource(RngSeed(save.rng_seed));
    world.insert_resource(GameRng(save.rng_state));
    world.insert_resource(FxRng::new(save.rng_seed));
    world.insert_resource(DungeonLord::default());

    // Rebuild the map from the seed rather than the save file, then restore the
    // dark-room mask so wand-of-light progress survives the reload.
    regenerate_map(world, save.rng_seed, save.depth);
    world.resource_mut::<Map>().dark = save.dark_tiles;

    // The deepest floor has no down-stair: the seed-built map still carries one,
    // so carve it back to plain floor. The Element of Yoord entity (or its place
    // in the pack) comes back from the save itself.
    if save.depth >= FINAL_DEPTH {
        let mut map = world.resource_mut::<Map>();
        if let Some(i) = map.tiles.iter().position(|&t| t == TileType::Downstairs) {
            map.tiles[i] = TileType::Room;
        }
    }

    let count = save.entities.len();
    let mut new_ents = Vec::with_capacity(count);
    for _ in 0..count {
        new_ents.push(world.spawn_empty().id());
    }

    // Who ends up wearing something, so their gear's loaned effects can be
    // re-lent once every entity exists.
    // Effects the save named that this build has no row for; reported once at
    // the end rather than per entity.
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
        let entity_name = es.name.map(|n| n.into_owned());
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
            em.insert(Mob { movement_type: m });
        }
        // Blood is not serialised: every creature (player and monsters) bleeds,
        // so it is simply re-attached on load.
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
        if let Some(effect) = es.ring {
            em.insert(Ring { effect });
            // What a ring lends its wearer — and what it does the moment it goes
            // on — is fixed by its catalog row, so both are read back from there
            // rather than stored in every save file.
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
        // What a thing does in flight, and what a bow lends the hand holding it,
        // are fixed by their catalog rows — the same as what a ring lends its
        // wearer. Read back from the table rather than stored in every save.
        if let Some(name) = entity_name.as_deref() {
            restore_from_catalog(&mut em, name);
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
        // A monster's innate grant list comes back from the bestiary; the
        // effects it actually has right now come back from the save, so a
        // cancelled dragon stays cancelled.
        if let Some(def) = entity_name.as_deref().and_then(MonsterDef::lookup) {
            if !def.grants.is_empty() {
                em.insert(Grants(def.grants));
            }
        }
        let held: Vec<Held> = es.effects.iter().filter_map(SavedEffect::held).collect();
        // Ids this build has no row for. `held` has already dropped them, so
        // this is the count of what the save knew and we do not.
        retired += es.effects.len() - held.len();
        attach_effects(&mut em, &held);
        // Every actor moves at some tempo; the energy pool starts fresh.
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
        if let Some(slots) = es.spellset {
            em.insert(Spellset { slots });
        }
    }

    // What gear lends its bearer comes back with the gear: the saved effect
    // sets leave loaned effects out, so they are re-attached here rather than
    // waiting for the first turn. The wearer list, not a pack query, drives
    // this — a monster that caught a thrown ring has no pack to be found by.
    for bearer in bearers {
        crate::equipment::sync_equipment_effects(world, bearer);
    }

    // An id the save carried and this build has no row for: a retired effect,
    // or one from a newer build. Dropping it loses one property rather than
    // the run, which is the whole reason effects are saved by name — a
    // positional format could not have told the difference, because every
    // index would still have been a valid index. Say so rather than letting
    // the player wonder why their ring went quiet.
    if retired > 0 {
        let line = match retired {
            1 => "One enchantment in this save is unknown to this build, and is gone.".to_string(),
            n => format!("{n} enchantments in this save are unknown to this build, and are gone."),
        };
        world.resource_mut::<GameLog>().add(line);
    }

    Ok(())
}
