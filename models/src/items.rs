use std::collections::HashSet;
use bevy_ecs::{entity::Entity, prelude::With, world::World};
use crossterm::style::Color;
use rand::Rng;
use std::collections::VecDeque;
use crate::{components::*, map::{tile_index, GameRng, Map, TileType, MAP_WIDTH, MAP_HEIGHT}, particles::Particles, helpers::{apply_damage, get_entities_at_position, get_line}};
use crate::effects::{revoke_all, ColdImmune, FireImmune, Grant, Undead};
use crate::equipment::{equipped_in, equipped_items, force_unequip, toggle_equipped, Slot};
use crate::identify::Identified;
use crate::magicmap::{MagicMapReveal, MagicMapStyle};
use crate::monsters::{spawn_monster, BESTIARY};
use crate::traps::{random_open_tile, Trap};

fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) {
    match effect {
        PotionEffect::Healing => {
            if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
                fighter.hp = std::cmp::min(fighter.hp + 10, fighter.max_hp);
                world.resource_mut::<GameLog>().add("Healing!".to_string());
            }
        }
        _ => { /* handle other potion effects */ }
    }
}

/// The three flavours of elemental wand. A creature can be immune to one — and
/// the immunity is a plain component, so a dragon's innate `FireImmune` and a
/// future ring of fire resistance's are the same thing to this code.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Element {
    Fire,
    Cold,
    Drain,
}

impl Element {
    /// Which element a wand's damage carries, if any. Non-elemental damage
    /// (magic missile, lightning, striking) returns `None` and is never resisted.
    fn of(effect: WandEffect) -> Option<Element> {
        match effect {
            WandEffect::Fire => Some(Element::Fire),
            WandEffect::Cold => Some(Element::Cold),
            WandEffect::DrainLife => Some(Element::Drain),
            _ => None,
        }
    }

    /// The effect that shrugs this element off.
    fn immunity(self) -> Grant {
        match self {
            Element::Fire => Grant::of::<FireImmune>(),
            Element::Cold => Grant::of::<ColdImmune>(),
            Element::Drain => Grant::of::<Undead>(),
        }
    }

    /// The word for this element in an "unharmed by the ___" log line.
    fn noun(self) -> &'static str {
        match self {
            Element::Fire => "flames",
            Element::Cold => "cold",
            Element::Drain => "evil magic",
        }
    }
}

/// Whether `entity` shrugs off `element`, from any source.
fn is_immune(world: &World, entity: Entity, element: Element) -> bool {
    element.immunity().probe(world, entity)
}

/// A wand's damage: `3d3`, rolled once per zap and applied whole to every
/// creature it touches (armour is never subtracted — see [`apply_damage`]).
fn roll_wand_damage(world: &mut World) -> i32 {
    let mut rng = world.resource_mut::<GameRng>();
    (0..3).map(|_| rng.0.gen_range(1..=3)).sum()
}

/// The hostile monster standing on `pos`, if any.
fn monster_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .find(|(_, p, f)| p.x == pos.x && p.y == pos.y && **f == Faction::Monster)
        .map(|(e, _, _)| e)
}

/// Applies `damage` of `element` (or non-elemental if `None`) to `entity`,
/// respecting immunity. Returns how much HP was actually taken off — 0 if the
/// creature resisted or had no [`Fighter`]. Immunity is logged for named
/// creatures.
fn damage_with_element(
    world: &mut World,
    entity: Entity,
    damage: i32,
    element: Option<Element>,
) -> i32 {
    if let Some(el) = element {
        if is_immune(world, entity, el) {
            if let Some(name) = world.get::<Name>(entity).map(|n| n.what.clone()) {
                world
                    .resource_mut::<GameLog>()
                    .add(format!("The {name} is unharmed by the {}.", el.noun()));
            }
            return 0;
        }
    }
    let Some(hp_before) = world.get::<Fighter>(entity).map(|f| f.hp) else {
        return 0;
    };
    apply_damage(world, entity, damage);
    damage.min(hp_before.max(0))
}

fn apply_wand_effect(world: &mut World, user: Entity, target: Option<Position>, effect: WandEffect) {
    let user_pos = match world.get::<Position>(user) {
        Some(pos) => *pos,
        None => return,
    };

    // The wand of light takes no target: it floods the room (or passage) the
    // zapper is standing in.
    if effect == WandEffect::Light {
        light_area(world, user, user_pos);
        return;
    }

    let target_pos = match target {
        Some(pos) => pos,
        None => return, // Safety catch: every other wand requires a target.
    };

    // Bolt wands: travel a straight line to the target, damaging everything on
    // the way. `None` means this effect isn't a damaging bolt. The colour is the
    // one the animated beam streaks in.
    let bolt: Option<(&str, Color)> = match effect {
        WandEffect::MagicMissile => Some(("A brilliant cyan bolt leaps from the wand!", Color::Cyan)),
        WandEffect::Lightning => Some(("A forking bolt of lightning cracks out!", Color::Yellow)),
        WandEffect::Striking => Some(("An invisible fist hammers down the line!", Color::White)),
        WandEffect::DrainLife => {
            Some(("A tendril of black light drinks the life from its path.", Color::DarkMagenta))
        }
        _ => None,
    };

    match effect {
        _ if bolt.is_some() => {
            let (msg, color) = bolt.unwrap();
            let element = Element::of(effect);
            let damage = roll_wand_damage(world);
            world.resource_mut::<GameLog>().add(msg.to_string());
            let map = world.resource::<Map>().clone();
            let line_points = get_line(user_pos, target_pos);
            let mut beam_cells: Vec<(u16, u16)> = Vec::new();
            let mut drained = 0;
            for pos in line_points {
                if map.blocks(pos.x, pos.y) {
                    break;
                }
                if !(pos.x == user_pos.x && pos.y == user_pos.y) {
                    beam_cells.push((pos.x, pos.y));
                }
                let entities_at_pos = get_entities_at_position(world, pos);
                for entity in entities_at_pos {
                    if entity != user {
                        drained += damage_with_element(world, entity, damage, element);
                    }
                }
            }
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.beam(&beam_cells, color);
            }
            // The wand of drain life feeds the life it takes straight back to the
            // zapper (never past their maximum).
            if effect == WandEffect::DrainLife && drained > 0 {
                if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
                    fighter.hp = (fighter.hp + drained).min(fighter.max_hp);
                }
                world
                    .resource_mut::<GameLog>()
                    .add(format!("You drain {drained} life."));
            }
        }
        WandEffect::Fire | WandEffect::Cold => {
            let is_fire = effect == WandEffect::Fire;
            let element = Element::of(effect);
            let msg = if is_fire {
                "A roaring sphere of fire erupts!"
            } else {
                "A blast of freezing air detonates!"
            };
            let damage = roll_wand_damage(world);
            world.resource_mut::<GameLog>().add(msg.to_string());

            // Radius of the blast disc, in tiles.
            let radius: f32 = 3.0;
            let map = world.resource::<Map>().clone();
            let cx = target_pos.x as i32;
            let cy = target_pos.y as i32;
            let r = radius.ceil() as i32;

            // Every tile within the disc that the blast centre has line of sight
            // to (walls stop the flames), tagged with its distance from centre
            // so the animation can ripple outward.
            let mut blast_cells: Vec<(u16, u16, f32)> = Vec::new();
            for dy in -r..=r {
                for dx in -r..=r {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist > radius {
                        continue;
                    }
                    let Some((tx, ty)) = crate::particles::on_map(cx + dx, cy + dy) else {
                        continue;
                    };
                    let ray = get_line(target_pos, Position { x: tx, y: ty });
                    let blocked = ray
                        .iter()
                        .any(|p| map.blocks(p.x, p.y) && !(p.x == tx && p.y == ty));
                    if !blocked {
                        blast_cells.push((tx, ty, dist));
                    }
                }
            }

            // Damage every fighter standing in a blast cell.
            let cell_set: std::collections::HashSet<(u16, u16)> =
                blast_cells.iter().map(|&(x, y, _)| (x, y)).collect();
            let mut affected_entities = Vec::new();
            let mut query = world.query::<(Entity, &Position)>();
            for (entity, pos) in query.iter(world) {
                if cell_set.contains(&(pos.x, pos.y)) {
                    affected_entities.push(entity);
                }
            }
            for entity in affected_entities {
                damage_with_element(world, entity, damage, element);
            }

            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.explosion(&blast_cells, is_fire);
            }
        }
        WandEffect::Polymorph => polymorph_target(world, target_pos),
        WandEffect::HasteMonster => shift_target_speed(world, target_pos, true),
        WandEffect::SlowMonster => shift_target_speed(world, target_pos, false),
        WandEffect::TeleportAway => teleport_target_away(world, target_pos),
        WandEffect::TeleportTo => teleport_target_here(world, user_pos, target_pos),
        WandEffect::Cancellation => cancel_target(world, target_pos),
        WandEffect::Nothing => {
            world
                .resource_mut::<GameLog>()
                .add("The wand does nothing. It was well named.".to_string());
        }
        // Light is handled above; the remaining arms are the damaging wands.
        WandEffect::Light => {}
        _ => {
            world
                .resource_mut::<GameLog>()
                .add("The wand discharges with a faint hiss.".to_string());
        }
    }
}

/// Wand of light: reveal — instantly — the whole room the zapper stands in (a
/// dark room is lit for good), or the whole passage if they are in a corridor.
/// Any hidden trap in the lit area comes to light too.
fn light_area(world: &mut World, user: Entity, from: Position) {
    let map = world.resource::<Map>().clone();
    let here = map.tile(from.x, from.y);
    let in_room = matches!(
        here,
        TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
    );

    // Flood-fill from the zapper's tile through tiles of the same "space": room
    // floor + doorways + stairs for a room, passage tiles for a corridor.
    let connects = |t: TileType| {
        if in_room {
            matches!(
                t,
                TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
            )
        } else {
            t == TileType::Passage
        }
    };

    let mut area: Vec<(u16, u16)> = Vec::new();
    let mut seen: std::collections::HashSet<(u16, u16)> = std::collections::HashSet::new();
    let mut queue: VecDeque<(u16, u16)> = VecDeque::new();
    queue.push_back((from.x, from.y));
    seen.insert((from.x, from.y));
    while let Some((cx, cy)) = queue.pop_front() {
        area.push((cx, cy));
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if seen.contains(&(nx, ny)) || !connects(map.tile(nx, ny)) {
                    continue;
                }
                seen.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }

    // Clear the dark flag and fold every lit tile into the player's memory.
    {
        let mut map_mut = world.resource_mut::<Map>();
        for &(x, y) in &area {
            map_mut.light_tile(x, y);
        }
    }
    if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
        if vs.revealed_tiles.len() < MAP_WIDTH as usize * MAP_HEIGHT as usize {
            vs.revealed_tiles.grow(MAP_WIDTH as usize * MAP_HEIGHT as usize);
        }
        for &(x, y) in &area {
            vs.revealed_tiles.insert(tile_index(x, y));
        }
        vs.dirty = true;
    }

    // Bring any hidden trap in the lit area to light.
    let lit: std::collections::HashSet<(u16, u16)> = area.iter().copied().collect();
    let sprung: Vec<(Entity, String)> = world
        .query_filtered::<(Entity, &Position, &Trap), With<Hidden>>()
        .iter(world)
        .filter(|(_, p, t)| !t.revealed && lit.contains(&(p.x, p.y)))
        .map(|(e, _, t)| (e, format!("{} {}", t.effect.label_article(), t.effect.label())))
        .collect();
    for (trap, label) in sprung {
        world.entity_mut(trap).remove::<Hidden>();
        if let Some(mut t) = world.get_mut::<Trap>(trap) {
            t.revealed = true;
        }
        world.resource_mut::<GameLog>().add(format!("The light reveals {label}!"));
    }

    let msg = if in_room {
        "Warm light floods the room."
    } else {
        "Light races the length of the passage."
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

/// Wand of polymorph: replace the monster on `pos` with a different species,
/// fresh, on the same tile.
fn polymorph_target(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add("The bolt of change fizzles against nothing.".to_string());
        return;
    };
    let old_name = item_label(world, victim);
    world.entity_mut(victim).despawn();

    // Roll a bestiary entry that isn't what we started with.
    let def = loop {
        let idx = {
            let mut rng = world.resource_mut::<GameRng>();
            rng.0.gen_range(0..BESTIARY.len())
        };
        let candidate = &BESTIARY[idx];
        if candidate.name != old_name {
            break candidate;
        }
    };
    let new_name = def.name.to_string();
    spawn_monster(world, def, pos);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {old_name} twists and warps into {} {new_name}!", crate::identify::article_for(&new_name)));
}

/// Wand of haste / slow monster: step the target one notch along the speed scale
/// (permanently).
fn shift_target_speed(world: &mut World, pos: Position, faster: bool) {
    let Some(victim) = monster_at(world, pos) else {
        world.resource_mut::<GameLog>().add("Nothing there to enchant.".to_string());
        return;
    };
    let name = item_label(world, victim);
    let Some(mut speed) = world.get_mut::<Speed>(victim) else { return };
    let before = speed.kind;
    speed.kind = if faster { before.faster() } else { before.slower() };
    let after = speed.kind;
    let msg = if after == before {
        format!("The {name} is already as {} as it can be.", if faster { "quick" } else { "sluggish" })
    } else if faster {
        format!("The {name} blurs into sudden speed.")
    } else {
        format!("The {name} lurches into slow motion.")
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Wand of teleport away: fling the target monster to a random open tile.
fn teleport_target_away(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world.resource_mut::<GameLog>().add("The wand's pull finds nothing.".to_string());
        return;
    };
    let name = item_label(world, victim);
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
    }
    world.resource_mut::<GameLog>().add(format!("The {name} is yanked away into the dark."));
}

/// Wand of teleport to: drag the target monster to a tile next to the zapper.
fn teleport_target_here(world: &mut World, user_pos: Position, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world.resource_mut::<GameLog>().add("The wand's pull finds nothing.".to_string());
        return;
    };
    let name = item_label(world, victim);
    let spot = free_adjacent_tile(world, user_pos).or_else(|| random_open_tile(world));
    if let Some((x, y)) = spot {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
    }
    world.resource_mut::<GameLog>().add(format!("The {name} is dragged to your side!"));
}

/// Wand of cancellation: strip every marker effect the target has and reset its
/// tempo to normal. It keeps its name, fighting stats, movement and everything
/// else that makes it a creature. Because it walks [`crate::effects::EFFECTS`],
/// a newly added effect is cancellable the moment it joins the registry.
fn cancel_target(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world.resource_mut::<GameLog>().add("The grey ray strikes only stone.".to_string());
        return;
    };
    let name = item_label(world, victim);
    revoke_all(world, victim);
    if let Some(mut speed) = world.get_mut::<Speed>(victim) {
        speed.kind = SpeedKind::Normal;
    }
    world.resource_mut::<GameLog>().add(format!("The {name}'s magic sputters and dies."));
}

/// Destroys every cursed item `user` currently has equipped (a scroll of remove
/// curse): each one is unequipped, pulled out of the pack and despawned. Cursed
/// items sitting unequipped in the pack are left untouched. Returns how many
/// items were destroyed.
pub(crate) fn lift_curses(world: &mut World, user: Entity) -> usize {
    let doomed: Vec<Entity> = equipped_items(world, user)
        .into_iter()
        .filter(|&e| world.get::<Curse>(e).is_some())
        .collect();

    for &e in &doomed {
        force_unequip(world, e);
        if let Some(mut bp) = world.get_mut::<Backpack>(user) {
            bp.items.retain(|&i| i != e);
        }
        world.entity_mut(e).despawn();
    }
    // The gear is gone, so whatever it was lending its wearer goes with it.
    crate::equipment::sync_equipment_effects(world, user);
    doomed.len()
}

/// An item's display name, or a vague fallback.
pub(crate) fn item_label(world: &World, item: Entity) -> String {
    world.get::<Name>(item).map(|n| n.what.clone()).unwrap_or_else(|| "item".to_string())
}

/// Picks a uniformly random item in `user`'s backpack whose true type isnwhich has no interactive item picker (yet).'t
/// identified yet and identifies it directly. Used by
/// [`ScrollEffect::Identify`], 
fn identify_random_unknown_item(world: &mut World, user: Entity) {
    let candidates: Vec<Entity> = world.get::<Backpack>(user).map(|bp| bp.items.clone()).unwrap_or_default();

    let is_unidentified = |world: &World, e: Entity| -> bool {
        let identified = world.resource::<Identified>();
        world.get::<Potion>(e).is_some_and(|p| !identified.potions.contains(&p.effect))
            || world.get::<Scroll>(e).is_some_and(|s| !identified.scrolls.contains(&s.effect))
            || world.get::<Wand>(e).is_some_and(|w| !identified.wands.contains(&w.effect))
            || world.get::<Ring>(e).is_some_and(|r| !identified.rings.contains(&r.effect))
    };

    let unknown: Vec<Entity> = candidates.into_iter().filter(|&e| is_unidentified(world, e)).collect();
    if unknown.is_empty() {
        world.resource_mut::<GameLog>().add("You already recognise everything in your pack.".to_string());
        return;
    }
    let target = {
        let mut rng = world.resource_mut::<GameRng>();
        unknown[rng.0.gen_range(0..unknown.len())]
    };

    let name = item_label(world, target);
    let potion_effect = world.get::<Potion>(target).map(|p| p.effect);
    let scroll_effect = world.get::<Scroll>(target).map(|s| s.effect);
    let wand_effect = world.get::<Wand>(target).map(|w| w.effect);
    let ring_effect = world.get::<Ring>(target).map(|r| r.effect);

    let mut identified = world.resource_mut::<Identified>();
    if let Some(effect) = potion_effect {
        identified.potions.insert(effect);
    }
    if let Some(effect) = scroll_effect {
        identified.scrolls.insert(effect);
    }
    if let Some(effect) = wand_effect {
        identified.wands.insert(effect);
    }
    if let Some(effect) = ring_effect {
        identified.rings.insert(effect);
    }
    drop(identified);

    world.resource_mut::<GameLog>().add(format!("The scroll identifies your {name}!"));
}

fn apply_scroll_effect(world: &mut World, user: Entity, effect: ScrollEffect) {
    if effect == ScrollEffect::Identify {
        identify_random_unknown_item(world, user);
        return;
    }
    if effect == ScrollEffect::RemoveCurse {
        let freed = lift_curses(world, user);
        let msg = if freed > 0 {
            "You feel as though somebody is watching over you. Your cursed gear crumbles away."
        } else {
            "You feel as though somebody is watching over you."
        };
        world.resource_mut::<GameLog>().add(msg.to_string());
        return;
    }
    if effect == ScrollEffect::MagicMapping {
        // Roll the wipe's shape (or take the `ROOG_MAGICMAP` dev override), then
        // arm it centred on the reader. The engine plays it out frame by frame
        // after the turn (see [`crate::magicmap`]); headless callers with no
        // reveal resource just skip the animation.
        let hero = world
            .get::<Position>(user)
            .map(|p| (p.x, p.y))
            .unwrap_or((MAP_WIDTH / 2, MAP_HEIGHT / 2));
        let style = std::env::var("ROOG_MAGICMAP")
            .ok()
            .and_then(|v| MagicMapStyle::from_name(&v))
            .unwrap_or_else(|| MagicMapStyle::roll(&mut world.resource_mut::<GameRng>().0));
        if let Some(mut reveal) = world.get_resource_mut::<MagicMapReveal>() {
            reveal.start(hero, style);
        }
        world.resource_mut::<GameLog>().add(style.flavour().to_string());
        return;
    }
    match effect {
        ScrollEffect::Teleportation => teleport_reader(world, user),
        ScrollEffect::AggravateMonsters => aggravate_floor(world, user),
        ScrollEffect::CreateMonster => create_monster(world, user),
        ScrollEffect::ScareMonster => {
            let scared = scare_in_view(world, user);
            let msg = if scared > 0 {
                "The parchment flares with the pathos of fear!"
            } else {
                "The parchment radiates a menacing aura, but nothing is here to feel it."
            };
            world.resource_mut::<GameLog>().add(msg.to_string());
        }
        ScrollEffect::VorpalizeWeapon => vorpalize_wielded_weapon(world, user),
        ScrollEffect::BlankPaper => {
            world
                .resource_mut::<GameLog>()
                .add("The scroll is blank. Someone got the last laugh.".to_string());
        }
        _ => {
            world
                .resource_mut::<GameLog>()
                .add("You read the scroll, but nothing obvious happens.".to_string());
        }
    }
}

/// Scroll of teleportation: whisk the reader to a random open tile somewhere on
/// the current floor.
fn teleport_reader(world: &mut World, user: Entity) {
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut pos) = world.get_mut::<Position>(user) {
            pos.x = x;
            pos.y = y;
        }
        if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
            vs.dirty = true;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add("BLONK! You are whisked away!".to_string());
}

/// Scroll of aggravate monsters: every creature on the floor drops what it was
/// doing and homes in on the reader's tile — in or out of sight. See
/// [`MovementType::Aggravated`].
fn aggravate_floor(world: &mut World, user: Entity) {
    aggravate_all_monsters(world, user);
    world
        .resource_mut::<GameLog>()
        .add("A shrill shriek rips through the dungeon. Everything on this floor heard it — and it knows where you are.".to_string());
}

/// The bare mechanic: point every hostile on the floor at `origin`'s tile. The
/// scroll of aggravate monsters wraps this in its own flavour; so does the
/// [`crate::effects::AggravatesMonsters`] passive (see [`crate::abilities`]).
pub(crate) fn aggravate_all_monsters(world: &mut World, origin: Entity) {
    let Some(&hero) = world.get::<Position>(origin) else { return };
    let mobs: Vec<Entity> = world
        .query_filtered::<Entity, With<Mob>>()
        .iter(world)
        .collect();
    for m in mobs {
        if world.get::<Faction>(m) != Some(&Faction::Monster) {
            continue;
        }
        if let Some(mut mob) = world.get_mut::<Mob>(m) {
            mob.movement_type = MovementType::Aggravated { tx: hero.x, ty: hero.y };
        }
    }
}

/// Scroll of scare monster: every monster currently in the reader's view turns
/// tail for good. Returns how many were scared.
fn scare_in_view(world: &mut World, user: Entity) -> usize {
    let seen: HashSet<(u16, u16)> = world
        .get::<Viewshed>(user)
        .map(|v| v.visible_tiles.iter().copied().collect())
        .unwrap_or_default();
    let targets: Vec<Entity> = world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .filter(|(_, p, f)| **f == Faction::Monster && seen.contains(&(p.x, p.y)))
        .map(|(e, _, _)| e)
        .collect();
    for t in &targets {
        if let Some(mut mob) = world.get_mut::<Mob>(*t) {
            mob.movement_type = MovementType::Flee;
        }
    }
    targets.len()
}

/// A free walkable tile next to `origin` that no entity is standing on, chosen
/// at random. `None` if the reader is boxed in.
fn free_adjacent_tile(world: &mut World, origin: Position) -> Option<(u16, u16)> {
    let occupied: HashSet<(u16, u16)> = world
        .query::<&Position>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .collect();
    let opts: Vec<(u16, u16)> = {
        let map = world.resource::<Map>();
        let mut v = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (origin.x as i32 + dx, origin.y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if !map.blocks(nx, ny) && !occupied.contains(&(nx, ny)) {
                    v.push((nx, ny));
                }
            }
        }
        v
    };
    if opts.is_empty() {
        return None;
    }
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..opts.len());
    Some(opts[idx])
}

/// Scroll of create monster: conjure any creature from the bestiary next to the
/// reader (or, failing an open adjacent tile, anywhere on the floor).
fn create_monster(world: &mut World, user: Entity) {
    let origin = world.get::<Position>(user).copied();
    let spot = origin
        .and_then(|o| free_adjacent_tile(world, o))
        .or_else(|| random_open_tile(world));
    let Some((x, y)) = spot else {
        world
            .resource_mut::<GameLog>()
            .add("The air curdles — then settles. Whatever was coming thought better of it.".to_string());
        return;
    };
    let idx = {
        let mut rng = world.resource_mut::<GameRng>();
        rng.0.gen_range(0..BESTIARY.len())
    };
    let e = spawn_monster(world, &BESTIARY[idx], Position { x, y });
    let name = item_label(world, e);
    world
        .resource_mut::<GameLog>()
        .add(format!("The air curdles into {} {name}, teeth and all!", crate::identify::article_for(&name)));
}

/// Scroll of vorpalize weapon: brand the reader's wielded weapon [`Vorpal`]
/// against one random species (it already bites clean through any Jabberwock).
/// A weapon can only take the edge once — read it over an already-vorpal weapon
/// and the blade can't hold the second enchantment: it crumbles to nothing.
fn vorpalize_wielded_weapon(world: &mut World, user: Entity) {
    let weapon = equipped_in(world, user, Slot::Hand);
    let Some(weapon) = weapon else {
        world
            .resource_mut::<GameLog>()
            .add("The scroll gutters out, failing to brand a weapon.".to_string());
        return;
    };
    if world.get::<Vorpal>(weapon).is_some() {
        let wname = item_label(world, weapon);
        force_unequip(world, weapon);
        if let Some(mut bp) = world.get_mut::<Backpack>(user) {
            bp.items.retain(|&i| i != weapon);
        }
        world.entity_mut(weapon).despawn();
        world
            .resource_mut::<GameLog>()
            .add(format!("The {wname} screams in pain and crumbles to dust."));
        return;
    }
    let bane = {
        let mut rng = world.resource_mut::<GameRng>();
        BESTIARY[rng.0.gen_range(0..BESTIARY.len())].name.to_string()
    };
    world.entity_mut(weapon).insert(Vorpal { bane: bane.clone() });
    let wname = item_label(world, weapon);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {wname} sings with a razor light, an omen of death to any {bane}."));
}

pub fn item_system(world: &mut World) {
    let mut use_queue = world.resource_mut::<UseQueue>();
    let uses = std::mem::take(&mut use_queue.uses);
    drop(use_queue);

    for item_use in uses {
        // What the player sees right now (appearance if unidentified, true
        // name otherwise) and the item's true name, captured before any
        // despawn below could make `item_use.item` unqueryable.
        let seen_name = crate::identify::display_name(world, item_use.item);
        let true_name = item_label(world, item_use.item);

        // We store the "work to be done" here
        let mut potion_effect: Option<PotionEffect> = None;
        let mut wand_effect: Option<WandEffect> = None;
        let mut scroll_effect: Option<ScrollEffect> = None;
        let mut is_equipment = false;
        let mut destroy_item = false;
        let mut return_to_inventory = false;

        {
            let mut item_entity = world.entity_mut(item_use.item);
            
            // Check for Potion
            if let Some(p) = item_entity.get::<Potion>() {
                potion_effect = Some(p.effect);
            }

            // Check for Wand
            if let Some(w) = item_entity.get::<Wand>() {
                wand_effect = Some(w.effect);
            }

            // Check for Scroll
            if let Some(s) = item_entity.get::<Scroll>() {
                scroll_effect = Some(s.effect);
            }

            // Equipment: using it toggles the equipped state (handled below).
            // Weapon, armour or ring — the slot on the component says which, and
            // nothing here needs to.
            if item_entity.get::<crate::equipment::Equipped>().is_some() {
                is_equipment = true;
            }

            // Handle Wands / Battery logic
            if let Some(mut battery) = item_entity.get_mut::<Battery>() {
                battery.charges -= 1;
                if battery.charges <= 0 {
                    destroy_item = true;
                } else {
                    // Item survives! We need to put it back in the user's bag.
                    return_to_inventory = true;
                }
            }

            // Handle basic consumables
            if let Some(_consume) = item_entity.get::<Consume>() {
                destroy_item = true;
            }
        } // Drop the entity_mut borrow so we can freely use the world again

        // 0. Equipment toggle — these items always go back in the pack.
        if is_equipment {
            toggle_equipped(world, item_use.user, item_use.item);
            return_to_inventory = true;
        }

        // Anything the game doesn't know how to "use" is handed straight back
        // rather than vanishing into limbo.
        if !destroy_item
            && !return_to_inventory
            && potion_effect.is_none()
            && wand_effect.is_none()
            && scroll_effect.is_none()
        {
            let name = crate::identify::with_the(&item_label(world, item_use.item));
            world.resource_mut::<GameLog>().add(format!("You can't use {name} right now."));
            return_to_inventory = true;
        }

        // 1. Manage the item's physical existence
        if return_to_inventory {
            if let Some(mut backpack) = world.get_mut::<Backpack>(item_use.user) {
                if let Some(idx) = item_use.slot_idx {
                    // Put it back in its exact slot (clamp if inventory shifted somehow)
                    let insert_pos = std::cmp::min(idx, backpack.items.len());
                    backpack.items.insert(insert_pos, item_use.item);
                } else {
                    backpack.items.push(item_use.item); // Fallback
                }
            }
        }

        if destroy_item {
            let is_wand = world.get::<Wand>(item_use.item).is_some();
            let is_potion = world.get::<Potion>(item_use.item).is_some();
            let is_scroll = world.get::<Scroll>(item_use.item).is_some();

            let mut log = world.resource_mut::<GameLog>();
            if is_wand {
                log.add(format!("The {seen_name} crumbles to dust!"));
            } else if is_potion {
                log.add(format!("You drink the {seen_name}."));
            } else if is_scroll {
                log.add(format!("You read the {seen_name}."));
            } else {
                log.add("The item turns to dust!".to_string());
            }

            world.entity_mut(item_use.item).despawn();
        } else if wand_effect.is_some() {
            // Wands survive a zap (until their battery runs dry, handled
            // above), so the "you use it" beat lives here instead.
            world.resource_mut::<GameLog>().add(format!("You zap the {seen_name}."));
        }

        // 2. Dispatch to specialized functions. Using a potion, scroll or
        // wand always identifies its true type — every roguelike's
        // use-to-identify convention (rings identify on wear instead, inside
        // `toggle_puton`).
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().potions.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
            if world.resource_mut::<Identified>().wands.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }

        if let Some(eff) = scroll_effect {
            apply_scroll_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().scrolls.insert(eff) {
                world.resource_mut::<GameLog>().add(format!("That was {} {true_name}!", crate::identify::article_for(&true_name)));
            }
        }
    }
}