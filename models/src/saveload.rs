use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;

use crate::components::*;
use crate::map::{regenerate_map, BloodStains, GameRng, RngSeed};
use crate::state::GameState;
use rand_chacha::ChaCha12Rng;

/// How many log lines to persist. `GameLog::add` already trims to this, but the
/// save path clamps too so an oversized in-memory log never bloats the file.
const LOG_CAP: usize = 10;

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
#[derive(Serialize, Deserialize)]
struct EntitySave<'a> {
    position: Option<(u16, u16)>,
    /// (glyph, palette index)
    renderable: Option<(char, u8)>,
    player: bool,
    hidden: bool,
    consume: bool,
    #[serde(borrow)]
    name: Option<Cow<'a, str>>,
    /// (range, fog-of-war bitset). `visible_tiles` is never saved: the
    /// visibility system rebuilds it on the first frame after load.
    viewshed: Option<(u16, FixedBitSet)>,
    /// (hp, max_hp, armor, power, armor_bonus, power_bonus)
    fighter: Option<(i32, i32, i32, i32, i32, i32)>,
    faction: Option<Faction>,
    /// Indices into the saved entity list.
    backpack: Option<Vec<u32>>,
    score: Option<i32>,
    mob: Option<MovementType>,
    #[serde(borrow)]
    item: Option<Cow<'a, str>>,
    value: Option<i32>,
    potion: Option<PotionEffect>,
    battery: Option<i8>,
    wand: Option<WandEffect>,
    ranged: Option<i32>,
    scroll: Option<ScrollEffect>,
    ring: Option<RingEffect>,
    /// (pow_increase, pow_bonus). `wielder` is always rebuilt as `None`.
    wield: Option<(i8, i8)>,
    /// (arm_increase, arm_bonus). `wearer` is always rebuilt as `None`.
    wear: Option<(i8, i8)>,
}

#[derive(Serialize, Deserialize)]
struct SaveGame<'a> {
    #[serde(borrow)]
    entities: Vec<EntitySave<'a>>,
    #[serde(borrow)]
    log_history: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    log_unread: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    player_name: Cow<'a, str>,
    /// The current dungeon depth.
    depth: u8,
    /// The seed the run was originally created from.
    rng_seed: u64,
    /// The live RNG state, so the stream continues exactly where it left off.
    rng_state: ChaCha12Rng,
}

/// Serializes the world to a compact postcard save file. The map is not saved:
/// it is rebuilt from the seed on load (see [`regenerate_map`]).
///
/// The save struct borrows everything it can (names, log lines, item labels)
/// straight out of the ECS, so no second copy of the world is built in RAM, and
/// the bytes are streamed to disk through a `BufWriter` rather than buffered.
pub fn save_game(world: &mut World, path: &str) -> std::io::Result<()> {
    let mut ents: Vec<Entity> = world.iter_entities().map(|e| e.id()).collect();
    ents.sort_by_key(|e| e.index());
    let index_map: HashMap<Entity, u32> =
        ents.iter().enumerate().map(|(i, e)| (*e, i as u32)).collect();

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
            consume: er.contains::<Consume>(),
            name: er.get::<Name>().map(|n| Cow::Borrowed(n.what.as_str())),
            viewshed: er
                .get::<Viewshed>()
                .map(|v| (v.range, v.revealed_tiles.clone())),
            fighter: er
                .get::<Fighter>()
                .map(|f| (f.hp, f.max_hp, f.armor, f.power, f.armor_bonus, f.power_bonus)),
            faction: er.get::<Faction>().copied(),
            backpack: er.get::<Backpack>().map(|b| {
                b.items
                    .iter()
                    .filter_map(|i| index_map.get(i).copied())
                    .collect()
            }),
            score: er.get::<Score>().map(|s| s.value),
            mob: er.get::<Mob>().map(|m| m.movement_type),
            item: er.get::<Item>().map(|it| Cow::Borrowed(it.name.as_str())),
            value: er.get::<Value>().map(|v| v.amount),
            potion: er.get::<Potion>().map(|p| p.effect),
            battery: er.get::<Battery>().map(|b| b.charges),
            wand: er.get::<Wand>().map(|w| w.effect),
            ranged: er.get::<Ranged>().map(|r| r.range),
            scroll: er.get::<Scroll>().map(|s| s.effect),
            ring: er.get::<PutOn>().map(|p| p.effect),
            wield: er.get::<Wield>().map(|w| (w.pow_increase, w.pow_bonus)),
            wear: er.get::<Wear>().map(|w| (w.arm_increase, w.arm_bonus)),
        });
    }

    let log = world.resource::<GameLog>();
    let hist_start = log.history.len().saturating_sub(LOG_CAP);
    let save = SaveGame {
        entities,
        log_history: log.history[hist_start..]
            .iter()
            .map(|s| Cow::Borrowed(s.as_str()))
            .collect(),
        log_unread: log.unread.iter().map(|s| Cow::Borrowed(s.as_str())).collect(),
        player_name: Cow::Borrowed(world.resource::<PlayerName>().what.as_str()),
        depth: world.resource::<Depth>().what,
        rng_seed: world.resource::<RngSeed>().0,
        rng_state: world.resource::<GameRng>().0.clone(),
    };

    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let mut writer = postcard::to_io(&save, writer)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writer.flush()
}

/// Rebuilds the world from a postcard save file. Inserts GameState, GameLog,
/// PlayerName and Depth resources; all other resources must already be present.
pub fn load_game(world: &mut World, path: &str) -> std::io::Result<()> {
    let bytes = std::fs::read(path)?;
    let save: SaveGame = postcard::from_bytes(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    world.insert_resource(GameState::new());
    world.insert_resource(BloodStains::new());
    world.insert_resource(GameLog {
        history: save.log_history.into_iter().map(Cow::into_owned).collect(),
        unread: save.log_unread.into_iter().map(Cow::into_owned).collect(),
    });
    world.insert_resource(PlayerName {
        what: save.player_name.into_owned(),
    });
    world.insert_resource(Depth { what: save.depth });
    world.insert_resource(RngSeed(save.rng_seed));
    world.insert_resource(GameRng(save.rng_state));

    // Rebuild the map from the seed rather than the save file.
    regenerate_map(world, save.rng_seed);

    let count = save.entities.len();
    let mut new_ents = Vec::with_capacity(count);
    for _ in 0..count {
        new_ents.push(world.spawn_empty().id());
    }

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
        if es.consume {
            em.insert(Consume);
        }
        if let Some(n) = es.name {
            em.insert(Name {
                what: n.into_owned(),
            });
        }
        if let Some((range, revealed_tiles)) = es.viewshed {
            em.insert(Viewshed {
                visible_tiles: Vec::new(),
                revealed_tiles,
                range,
                dirty: true,
            });
        }
        if let Some((hp, max_hp, armor, power, armor_bonus, power_bonus)) = es.fighter {
            em.insert(Fighter {
                hp,
                max_hp,
                armor,
                power,
                armor_bonus,
                power_bonus,
            });
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
        if let Some(it) = es.item {
            em.insert(Item {
                name: it.into_owned(),
            });
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
            em.insert(PutOn { bearer: None, effect });
        }
        if let Some((pow_increase, pow_bonus)) = es.wield {
            em.insert(Wield { wielder: None, pow_increase, pow_bonus });
        }
        if let Some((arm_increase, arm_bonus)) = es.wear {
            em.insert(Wear { wearer: None, arm_increase, arm_bonus });
        }
    }

    Ok(())
}
