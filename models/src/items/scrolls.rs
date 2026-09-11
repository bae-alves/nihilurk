//! Reading a scroll.
//!
//! The catalog ([`crate::catalog::SCROLLS`]) names each scroll; this file is
//! where the words on it take effect, keyed by [`ScrollEffect`]. A scroll read
//! aloud by a *monster* you threw it at runs the same [`apply_scroll_effect`],
//! with the monster as the reader (see [`super::throwing`]).

use bevy_ecs::{entity::Entity, prelude::With, world::World};
use rand::Rng;
use std::collections::HashSet;

use crate::components::*;
use crate::effects::{ArmorBonus, PowerBonus, ThrowBonus};
use crate::equipment::{Slot, equipped_in, equipped_items, force_unequip, sync_equipment_effects};
use crate::helpers::{free_adjacent_tile, item_label};
use crate::identify::Identified;
use crate::magicmap::{MagicMapReveal, MagicMapStyle};
use crate::map::{GameRng, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::{BESTIARY, spawn_monster};
use crate::traps::random_open_tile;

/// Destroys every cursed item `user` currently has equipped (a scroll of remove
/// curse): each one is unequipped, pulled out of the pack and despawned. Cursed
/// items sitting unequipped in the pack are left untouched. Returns how many
/// items were destroyed.
pub(super) fn lift_curses(world: &mut World, user: Entity) -> usize {
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
    sync_equipment_effects(world, user);
    doomed.len()
}

/// Whether `e` is a weapon, suit of armour or launcher with an enchantment
/// plus or a curse still hidden — the one thing a scroll of identify can teach
/// about it that isn't already covered by [`Identified`]. Skips gear with
/// nothing to reveal (a plain +0, uncursed item) so the scroll never burns
/// itself on a target that would look no different afterward.
fn has_hidden_quality(world: &World, e: Entity) -> bool {
    if world.get::<KnownQuality>(e).is_some() {
        return false;
    }
    world.get::<Curse>(e).is_some()
        || world.get::<PowerBonus>(e).is_some_and(|b| b.0 != 0)
        || world.get::<ArmorBonus>(e).is_some_and(|b| b.0 != 0)
        || world.get::<ThrowBonus>(e).is_some_and(|b| b.0 != 0)
}

/// Picks a uniformly random item in `user`'s backpack that still has something
/// to learn — a potion, scroll, wand or ring whose true type isn't identified,
/// or a piece of gear whose plus/curse isn't ([`has_hidden_quality`]) — and
/// identifies it directly. Used by [`ScrollEffect::Identify`], which has no
/// interactive item picker (yet).
fn identify_random_unknown_item(world: &mut World, user: Entity) {
    let candidates: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();

    let is_unidentified = |world: &World, e: Entity| -> bool {
        let identified = world.resource::<Identified>();
        world
            .get::<Potion>(e)
            .is_some_and(|p| !identified.potions.contains(&p.effect))
            || world
                .get::<Scroll>(e)
                .is_some_and(|s| !identified.scrolls.contains(&s.effect))
            || world
                .get::<Wand>(e)
                .is_some_and(|w| !identified.wands.contains(&w.effect))
            || world
                .get::<Ring>(e)
                .is_some_and(|r| !identified.rings.contains(&r.effect))
            || has_hidden_quality(world, e)
    };

    let unknown: Vec<Entity> = candidates
        .into_iter()
        .filter(|&e| is_unidentified(world, e))
        .collect();
    if unknown.is_empty() {
        world
            .resource_mut::<GameLog>()
            .add("You already recognise everything in your pack.".to_string());
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
    world.entity_mut(target).insert(KnownQuality);

    world
        .resource_mut::<GameLog>()
        .add(format!("The scroll identifies your {name}!"));
}

pub(super) fn apply_scroll_effect(world: &mut World, user: Entity, effect: ScrollEffect) {
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
        world
            .resource_mut::<GameLog>()
            .add(style.flavour().to_string());
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
pub(super) fn teleport_reader(world: &mut World, user: Entity) {
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
    let Some(&hero) = world.get::<Position>(origin) else {
        return;
    };
    let mobs: Vec<Entity> = world
        .query_filtered::<Entity, With<Mob>>()
        .iter(world)
        .collect();
    for m in mobs {
        if world.get::<Faction>(m) != Some(&Faction::Monster) {
            continue;
        }
        if let Some(mut mob) = world.get_mut::<Mob>(m) {
            mob.movement_type = MovementType::Aggravated {
                tx: hero.x,
                ty: hero.y,
            };
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

/// Scroll of create monster: conjure any creature from the bestiary next to the
/// reader (or, failing an open adjacent tile, anywhere on the floor).
fn create_monster(world: &mut World, user: Entity) {
    let origin = world.get::<Position>(user).copied();
    let spot = origin
        .and_then(|o| free_adjacent_tile(world, o))
        .or_else(|| random_open_tile(world));
    let Some((x, y)) = spot else {
        world.resource_mut::<GameLog>().add(
            "The air curdles — then settles. Whatever was coming thought better of it.".to_string(),
        );
        return;
    };
    let idx = {
        let mut rng = world.resource_mut::<GameRng>();
        rng.0.gen_range(0..BESTIARY.len())
    };
    let e = spawn_monster(world, &BESTIARY[idx], Position { x, y });
    let name = item_label(world, e);
    world.resource_mut::<GameLog>().add(format!(
        "The air curdles into {} {name}, teeth and all!",
        crate::identify::article_for(&name)
    ));
}

/// Scroll of vorpalize weapon: brand the reader's wielded weapon [`Vorpal`]
/// against one random species (it already bites clean through any Jabberwock).
/// A weapon can only take the edge once — read it over an already-vorpal weapon
/// and the blade can't hold the second enchantment: it crumbles to nothing.
///
/// A bow or crossbow in hand doesn't count — the edge has nothing to bite
/// with — so it fizzles exactly as if the hand were empty.
fn vorpalize_wielded_weapon(world: &mut World, user: Entity) {
    let weapon =
        equipped_in(world, user, Slot::Hand).filter(|&e| world.get::<Launcher>(e).is_none());
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
        BESTIARY[rng.0.gen_range(0..BESTIARY.len())]
            .name
            .to_string()
    };
    world
        .entity_mut(weapon)
        .insert(Vorpal { bane: bane.clone() });
    let wname = item_label(world, weapon);
    world.resource_mut::<GameLog>().add(format!(
        "The {wname} sings with a razor light, an omen of death to any {bane}."
    ));
}
