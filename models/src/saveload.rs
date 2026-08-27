use bevy_ecs::prelude::*;
use crossterm::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::components::*;
use crate::map::{regenerate_map, GameRng, RngSeed};
use crate::state::GameState;
use rand_chacha::ChaCha12Rng;

fn color_to_str(c: &Color) -> String {
    format!("{:?}", c)
}

fn str_to_color(s: &str) -> Color {
    match s {
        "Black" => Color::Black,
        "DarkGrey" => Color::DarkGrey,
        "Red" => Color::Red,
        "DarkRed" => Color::DarkRed,
        "Green" => Color::Green,
        "DarkGreen" => Color::DarkGreen,
        "Yellow" => Color::Yellow,
        "DarkYellow" => Color::DarkYellow,
        "Blue" => Color::Blue,
        "DarkBlue" => Color::DarkBlue,
        "Magenta" => Color::Magenta,
        "DarkMagenta" => Color::DarkMagenta,
        "Cyan" => Color::Cyan,
        "DarkCyan" => Color::DarkCyan,
        "White" => Color::White,
        "Grey" => Color::Grey,
        _ => Color::White,
    }
}

#[derive(Serialize, Deserialize)]
struct RenderableSave {
    glyph: char,
    color: String,
}

#[derive(Serialize, Deserialize)]
struct ViewshedSave {
    visible_tiles: Vec<(u16, u16)>,
    revealed_tiles: Vec<(u16, u16)>,
    range: u16,
}

#[derive(Serialize, Deserialize, Default)]
struct EntitySave {
    position: Option<(u16, u16)>,
    renderable: Option<RenderableSave>,
    #[serde(default)]
    player: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    consume: bool,
    name: Option<String>,
    viewshed: Option<ViewshedSave>,
    fighter: Option<(i32, i32, i32, i32)>,
    faction: Option<Faction>,
    backpack: Option<Vec<usize>>,
    score: Option<i32>,
    mob: Option<MovementType>,
    item: Option<String>,
    value: Option<i32>,
    potion: Option<PotionEffect>,
    battery: Option<i8>,
    wand: Option<WandEffect>,
    ranged: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct SaveGame {
    entities: Vec<EntitySave>,
    log_history: Vec<String>,
    log_unread: Vec<String>,
    player_name: String,
    /// The seed the run was originally created from.
    rng_seed: u64,
    /// The live RNG state, so the stream continues exactly where it left off.
    rng_state: ChaCha12Rng,
}

fn is_map_tile(e: &bevy_ecs::world::EntityRef) -> bool {
    e.contains::<Wall>() || e.contains::<Room>() || e.contains::<Passage>() || e.contains::<Door>()
}

/// Serializes the world to a JSON save file. Map tile entities are omitted: they
/// are rebuilt from the seed on load (see [`regenerate_map`]).
pub fn save_game(world: &mut World, path: &str) -> std::io::Result<()> {
    let mut ents: Vec<Entity> = world
        .iter_entities()
        .filter(|e| !is_map_tile(e))
        .map(|e| e.id())
        .collect();
    ents.sort_by_key(|e| e.index());
    let index_map: HashMap<Entity, usize> =
        ents.iter().enumerate().map(|(i, e)| (*e, i)).collect();

    let mut entities = Vec::with_capacity(ents.len());
    for &e in &ents {
        let er = world.entity(e);
        let mut es = EntitySave::default();

        if let Some(p) = er.get::<Position>() {
            es.position = Some((p.x, p.y));
        }
        if let Some(r) = er.get::<Renderable>() {
            es.renderable = Some(RenderableSave {
                glyph: r.glyph,
                color: color_to_str(&r.color),
            });
        }
        es.player = er.contains::<Player>();
        es.hidden = er.contains::<Hidden>();
        es.consume = er.contains::<Consume>();
        if let Some(n) = er.get::<Name>() {
            es.name = Some(n.what.clone());
        }
        if let Some(v) = er.get::<Viewshed>() {
            es.viewshed = Some(ViewshedSave {
                visible_tiles: v.visible_tiles.clone(),
                revealed_tiles: v.revealed_tiles.iter().copied().collect(),
                range: v.range,
            });
        }
        if let Some(f) = er.get::<Fighter>() {
            es.fighter = Some((f.hp, f.max_hp, f.armor, f.power));
        }
        if let Some(f) = er.get::<Faction>() {
            es.faction = Some(*f);
        }
        if let Some(b) = er.get::<Backpack>() {
            es.backpack = Some(
                b.items
                    .iter()
                    .filter_map(|i| index_map.get(i).copied())
                    .collect(),
            );
        }
        if let Some(s) = er.get::<Score>() {
            es.score = Some(s.value);
        }
        if let Some(m) = er.get::<Mob>() {
            es.mob = Some(m.movement_type);
        }
        if let Some(it) = er.get::<Item>() {
            es.item = Some(it.name.clone());
        }
        if let Some(v) = er.get::<Value>() {
            es.value = Some(v.amount);
        }
        if let Some(p) = er.get::<Potion>() {
            es.potion = Some(p.effect);
        }
        if let Some(b) = er.get::<Battery>() {
            es.battery = Some(b.charges);
        }
        if let Some(w) = er.get::<Wand>() {
            es.wand = Some(w.effect);
        }
        if let Some(r) = er.get::<Ranged>() {
            es.ranged = Some(r.range);
        }

        entities.push(es);
    }

    let log = world.resource::<GameLog>();
    let save = SaveGame {
        entities,
        log_history: log.history.clone(),
        log_unread: log.unread.clone(),
        player_name: world.resource::<PlayerName>().what.clone(),
        rng_seed: world.resource::<RngSeed>().0,
        rng_state: world.resource::<GameRng>().0.clone(),
    };

    let json = serde_json::to_string_pretty(&save)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

/// Rebuilds the world from a JSON save file. Inserts GameState, GameLog and
/// PlayerName resources; all other resources must already be present.
pub fn load_game(world: &mut World, path: &str) -> std::io::Result<()> {
    let json = std::fs::read_to_string(path)?;
    let save: SaveGame = serde_json::from_str(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    world.insert_resource(GameState::new());
    world.insert_resource(GameLog {
        history: save.log_history,
        unread: save.log_unread,
    });
    world.insert_resource(PlayerName {
        what: save.player_name,
    });
    world.insert_resource(RngSeed(save.rng_seed));
    world.insert_resource(GameRng(save.rng_state));

    // Rebuild the map from the seed rather than the save file.
    regenerate_map(world, save.rng_seed);

    let mut new_ents = Vec::with_capacity(save.entities.len());
    for _ in &save.entities {
        new_ents.push(world.spawn_empty().id());
    }

    for (i, es) in save.entities.iter().enumerate() {
        let mut em = world.entity_mut(new_ents[i]);

        if let Some((x, y)) = es.position {
            em.insert(Position { x, y });
        }
        if let Some(r) = &es.renderable {
            em.insert(Renderable {
                glyph: r.glyph,
                color: str_to_color(&r.color),
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
        if let Some(n) = &es.name {
            em.insert(Name { what: n.clone() });
        }
        if let Some(v) = &es.viewshed {
            em.insert(Viewshed {
                visible_tiles: v.visible_tiles.clone(),
                revealed_tiles: v.revealed_tiles.iter().copied().collect(),
                range: v.range,
                dirty: true,
            });
        }
        if let Some((hp, max_hp, armor, power)) = es.fighter {
            em.insert(Fighter {
                hp,
                max_hp,
                armor,
                power,
            });
        }
        if let Some(f) = es.faction {
            em.insert(f);
        }
        if let Some(items) = &es.backpack {
            let mapped: Vec<Entity> = items.iter().map(|&idx| new_ents[idx]).collect();
            em.insert(Backpack { items: mapped });
        }
        if let Some(s) = es.score {
            em.insert(Score { value: s });
        }
        if let Some(m) = es.mob {
            em.insert(Mob { movement_type: m });
        }
        if let Some(it) = &es.item {
            em.insert(Item { name: it.clone() });
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
    }

    Ok(())
}
