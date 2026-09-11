//! Hurling anything at anything.
//!
//! One entry point — [`throw_system`], the schedule step that drains the
//! [`ThrowQueue`] — and one resolver, [`resolve_throw`], that traces the item's
//! flight and works out what it does when it arrives. A potion shatters and is
//! drunk; a scroll is read aloud by anything literate; a wand bursts, spending
//! its whole battery at once (borrowing [`super::wands`]'s blast machinery); a
//! weapon hits, and a monster clever enough may catch it and use it on you.
//!
//! [`throw_refusal`] / [`drop_refusal`] are the "you can't" checks the engine
//! runs before it ever queues a throw.

use bevy_ecs::{entity::Entity, world::World};
use crossterm::style::Color;
use rand::Rng;

use crate::components::*;
use crate::effects::*;
use crate::equipment::{Equipped, Slot, equip_silently, force_unequip, sync_equipment_effects};
use crate::helpers::{actor_at, apply_damage, get_line, item_label, roll_dice, total_armor_plus};
use crate::identify::Identified;
use crate::map::{GameRng, Map};
use crate::particles::Particles;

use super::potions::apply_potion_effect;
use super::scrolls::apply_scroll_effect;
use super::wands::{
    blast_palette, cancel_entity, dazzle, elemental_blast, is_attack_wand, polymorph_entity,
    shift_entity_speed, teleport_entity_away, teleport_entity_to_self,
};

use crate::constants::items::{PACK_CAPACITY, THROW_RANGE};
use crate::constants::loot::LAUNCHER_DIE_MULTIPLIER;
use crate::constants::wands::{
    BLAST_RADIUS, EFFECT_DIE_PER_CHARGE, GRENADE_DIE_PER_CHARGE, GRENADE_RADIUS,
};

/// Why `user` can't throw `item`, if they can't. Two things stay in the pack:
/// the Element of Yoord, which is the whole point of the run and is not to be
/// flung down a corridor, and cursed gear, which is welded on — it won't come
/// off, so it can be neither dropped nor hurled. Anything else is fair game.
pub fn throw_refusal(world: &World, user: Entity, item: Entity) -> Option<String> {
    if world.get::<Amulet>(item).is_some() {
        return Some("The Element of Yoord will not leave your hand.".to_string());
    }
    drop_refusal(world, user, item)
}

/// Why `user` can't put `item` down, if they can't. Cursed gear is welded on;
/// the Element of Yoord, unlike a thrown one, *can* be set down — abandoning the
/// run's prize on the floor is the player's business.
pub fn drop_refusal(world: &World, user: Entity, item: Entity) -> Option<String> {
    let equipped = world.get::<Equipped>(item)?;
    if equipped.by != Some(user) || world.get::<Curse>(item).is_none() {
        return None;
    }
    Some(
        equipped
            .slot
            .stuck(&crate::identify::display_name(world, item)),
    )
}

/// Marks `item`'s true type as known, announcing it the same way using one
/// yourself does. Watching a monster drink, read or put on what you threw at it
/// teaches you exactly as much as doing it would have.
fn identify_from_afar(world: &mut World, item: Entity) {
    let true_name = item_label(world, item);
    let potion = world.get::<Potion>(item).map(|p| p.effect);
    let scroll = world.get::<Scroll>(item).map(|s| s.effect);
    let wand = world.get::<Wand>(item).map(|w| w.effect);
    let ring = world.get::<Ring>(item).map(|r| r.effect);
    let mut known = world.resource_mut::<Identified>();
    let newly = match (potion, scroll, wand, ring) {
        (Some(e), _, _, _) => known.potions.insert(e),
        (_, Some(e), _, _) => known.scrolls.insert(e),
        (_, _, Some(e), _) => known.wands.insert(e),
        (_, _, _, Some(e)) => known.rings.insert(e),
        _ => false,
    };
    if newly {
        world.resource_mut::<GameLog>().add(format!(
            "That was {} {true_name}!",
            crate::identify::article_for(&true_name)
        ));
    }
}

/// Traces a throw: the tiles the item crosses (the thrower's own excluded), the
/// tile it comes to rest on, and every creature it ran into, in the order it
/// reached them.
///
/// A wall always stops it short of the aimed spot. So does the first creature in
/// the way — which is the whole point of aiming past one — unless the thing in
/// flight is [`Piercing`], in which case it runs the line to its end and the
/// list comes back with everyone standing in it.
fn flight_path(
    world: &mut World,
    thrower: Entity,
    item: Entity,
    from: Position,
    to: Position,
) -> (Vec<(u16, u16)>, Position, Vec<Entity>) {
    let map = world.resource::<Map>().clone();
    let piercing = world.get::<Piercing>(item).is_some();
    let mut cells = Vec::new();
    let mut landing = from;
    let mut victims = Vec::new();
    for pos in get_line(from, to) {
        if pos == from {
            continue;
        }
        if map.blocks(pos.x, pos.y) {
            break;
        }
        cells.push((pos.x, pos.y));
        landing = pos;
        if let Some(victim) = actor_at(world, pos, thrower) {
            victims.push(victim);
            if !piercing {
                break;
            }
        }
    }
    (cells, landing, victims)
}

/// What a hurled object does on impact, or `None` if it is not the sort of thing
/// that hurts anyone: only an item carrying [`ThrownDamage`] rolls at all, so a
/// wand or a suit of armour just bounces off and falls.
///
/// The roll is `1d[thrown damage]`, plus the item's own enchantment, plus every
/// [`ThrowBonus`] the *thrower* is wearing — a ring of dexterity, the plus on the
/// bow in their hand. Three things bend it:
///
/// * **A launcher doubles the die.** A missile carrying [`LaunchedBy`] asks
///   whether its thrower has the effect it answers to; an arrow lobbed by hand
///   rolls `1d4`, the same arrow loosed from a bow rolls `1d8`. The bow is not
///   consulted — only the effect is, so a monster that picked one up shoots just
///   as well as you do.
/// * **A [`Projectile`] ignores armour.** A point already in the air does not
///   care what you are wearing.
/// * **Anything else is still blunted by it** — by the armour *plus* only, never
///   the die, exactly as a trap is.
fn roll_throw_damage(
    world: &mut World,
    thrower: Entity,
    item: Entity,
    target: Entity,
) -> Option<i32> {
    let mut die = world.get::<ThrownDamage>(item)?.0;
    if let Some(&LaunchedBy(launcher)) = world.get::<LaunchedBy>(item) {
        if launcher.probe(world, thrower) {
            die *= LAUNCHER_DIE_MULTIPLIER;
        }
    }
    if die < 1 {
        return Some(0);
    }
    let bonus = world.get::<PowerBonus>(item).map(|b| b.0).unwrap_or(0)
        + equipped_total::<ThrowBonus>(world, thrower);
    let roll = world.resource_mut::<GameRng>().0.gen_range(1..=die) + bonus;
    let soak = match world.get::<Projectile>(item) {
        Some(_) => 0,
        None => total_armor_plus(world, target),
    };
    Some((roll - soak).max(0))
}

/// Lays a thrown item down on the floor where it stopped, ready to be picked up
/// again.
fn land_item(world: &mut World, item: Entity, at: Position) {
    world.entity_mut(item).insert(at);
}

/// The single missile a throw actually looses.
///
/// A quiver does not leave your hand when you shoot it: it gives up one arrow
/// and stays exactly where it was in the pack, one lighter — `slot` is the row
/// it came from, so it goes back there instead of to the bottom of the list.
/// Anything that is not a stack is thrown whole and comes back unchanged.
///
/// Called with `item` already lifted out of the pack, which is how both the
/// engine's Throw action and a scripted throw hand an item over.
pub fn draw_one(world: &mut World, thrower: Entity, item: Entity, slot: Option<usize>) -> Entity {
    let Some(count) = world.get::<Stack>(item).map(|s| s.count).filter(|&c| c > 1) else {
        return item;
    };
    let Some(one) = crate::catalog::split_one(world, item) else {
        return item;
    };
    if let Some(mut stack) = world.get_mut::<Stack>(item) {
        stack.count = count - 1;
    }
    if let Some(mut pack) = world.get_mut::<Backpack>(thrower) {
        let at = slot.unwrap_or(pack.items.len()).min(pack.items.len());
        pack.items.insert(at, item);
    }
    one
}

/// Takes `item` into `carrier`'s pack, and returns what was taken as it reads in
/// a sentence — `"a dagger"`, `"7 arrows"` — or `None` if there was no pack to
/// put it in, or the pack is already at [`PACK_CAPACITY`] and this item can't
/// merge into a slot already there.
///
/// Ammunition merges. A bundle off the floor tops up the quivers already in the
/// pack rather than claiming a fresh inventory letter for every arrow, and only
/// what is left over after they are all full to [`STACK_LIMIT`] takes a slot of
/// its own. Everything else claims its own slot, exactly as it always has.
pub fn stow(world: &mut World, carrier: Entity, item: Entity) -> Option<String> {
    let Some(mut left) = world.get::<Stack>(item).map(|s| s.count) else {
        let backpack = world.get::<Backpack>(carrier)?;
        if backpack.items.len() >= PACK_CAPACITY {
            return None;
        }
        let label = crate::identify::with_article(world, item);
        world.get_mut::<Backpack>(carrier)?.items.push(item);
        world.entity_mut(item).remove::<Position>();
        return Some(label);
    };

    // Every quiver of the same thing that still has room, in pack order.
    let name = world.get::<Name>(item)?.what.clone();
    let quivers: Vec<Entity> = world
        .get::<Backpack>(carrier)?
        .items
        .iter()
        .copied()
        .filter(|&e| {
            e != item
                && world.get::<Name>(e).is_some_and(|n| n.what == name)
                && world.get::<Stack>(e).is_some_and(|s| s.count < STACK_LIMIT)
        })
        .collect();

    let taking = left;
    for quiver in quivers {
        if left == 0 {
            break;
        }
        let Some(mut stack) = world.get_mut::<Stack>(quiver) else {
            continue;
        };
        let moved = left.min(STACK_LIMIT - stack.count);
        stack.count += moved;
        left -= moved;
    }

    match left {
        // Every last one went into a quiver: the pile has nothing left to be.
        0 => {
            world.entity_mut(item).despawn();
        }
        // The overflow takes a slot of its own rather than being left behind —
        // unless the pack has no slot left to give it, in which case what
        // didn't fit into a quiver stays behind on the floor.
        _ => {
            if let Some(mut stack) = world.get_mut::<Stack>(item) {
                stack.count = left;
            }
            let backpack = world.get::<Backpack>(carrier)?;
            if backpack.items.len() >= PACK_CAPACITY {
                return None;
            }
            world.get_mut::<Backpack>(carrier)?.items.push(item);
            world.entity_mut(item).remove::<Position>();
        }
    }
    Some(crate::identify::counted(&name, taking))
}

/// A thrown wand bursts where it lands, spending every charge it had left in one
/// go.
///
/// * An **attack** wand throws the wide, hot grenade — `d4` a charge at
///   [`GRENADE_RADIUS`], armour-ignoring, elemental where the wand is.
/// * The **wand of light** throws the same grenade (`d4` a charge, wide) but
///   [`dazzle`]s every creature it catches instead of carrying an element.
/// * A **utility** wand throws a smaller [`BLAST_RADIUS`] blast that deals *no*
///   damage — the payload is the effect, worked on every creature caught.
/// * The **wand of nothing** just makes confetti.
fn resolve_wand_throw(
    world: &mut World,
    item: Entity,
    landing: Position,
    effect: WandEffect,
    seen_name: &str,
) {
    let charges = world
        .get::<Battery>(item)
        .map(|b| b.charges as i32)
        .unwrap_or(0)
        .max(0);

    if effect == WandEffect::Nothing {
        world.resource_mut::<GameLog>().add(format!(
            "The {seen_name} bursts in a shower of colourful confetti. That's it. That's the whole spell."
        ));
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            confetti_burst(&mut fx, landing);
        }
        return;
    }

    let is_attack = is_attack_wand(effect);
    let is_light = effect == WandEffect::Light;
    // The wand of light throws the same wide, hot grenade an attack wand does —
    // it just blinds instead of burning through armour.
    let grenade = is_attack || is_light;
    let (radius, sides) = if grenade {
        (GRENADE_RADIUS, GRENADE_DIE_PER_CHARGE)
    } else {
        (BLAST_RADIUS, EFFECT_DIE_PER_CHARGE)
    };
    // Attack wands and the light wand deal damage; the utility wands' blasts are
    // pure delivery — the effect is the whole payload.
    let damage = if grenade {
        roll_dice(world, charges, sides)
    } else {
        0
    };
    let element = if is_attack { Element::of(effect) } else { None };

    world.resource_mut::<GameLog>().add(format!(
        "The {seen_name} shatters, and {charges} charges' worth of magic gets out at once!"
    ));

    let palette = blast_palette(effect);
    let caught = elemental_blast(world, landing, radius, damage, element, palette);

    if is_attack {
        return;
    }
    for entity in caught {
        // Only creatures answer to a wand's effect — a scroll lying in the blast
        // is not "confused". And an earlier victim may already have been
        // teleported clear or replaced outright.
        let is_creature =
            world.get::<Mob>(entity).is_some() || world.get::<Player>(entity).is_some();
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };
        if !is_creature {
            continue;
        }
        if is_light {
            dazzle(world, entity);
        } else {
            apply_thrown_wand_effect(world, entity, effect);
        }
        // A cosmetic-only echo confirming the effect actually landed on this
        // creature — no gameplay rides on it, just the darker follow-up pop.
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.secondary_burst(pos.x, pos.y, radius, palette);
        }
    }
}

/// The per-creature half of a thrown utility wand: the same change a zap would
/// have made, applied to everyone the blast reached.
fn apply_thrown_wand_effect(world: &mut World, entity: Entity, effect: WandEffect) {
    match effect {
        WandEffect::Polymorph => polymorph_entity(world, entity),
        WandEffect::HasteMonster => shift_entity_speed(world, entity, true),
        WandEffect::SlowMonster => shift_entity_speed(world, entity, false),
        WandEffect::TeleportAway => teleport_entity_away(world, entity),
        WandEffect::TeleportTo => teleport_entity_to_self(world, entity),
        WandEffect::Cancellation => cancel_entity(world, entity),
        _ => {}
    }
}

/// The schedule step that resolves everything hurled this turn.
pub fn throw_system(world: &mut World) {
    let throws = std::mem::take(&mut world.resource_mut::<ThrowQueue>().throws);
    for throw in throws {
        resolve_throw(world, throw);
    }
}

/// One thrown item, from the thrower's hand to whatever it finds along its line.
/// A potion shatters over its target and is drunk by it; a scroll is read aloud
/// by anything literate enough and otherwise flutters to the floor; everything
/// else simply arrives — hurting what it hits only if it carries
/// [`ThrownDamage`], and staying with a creature that knows what to do with it
/// ([`ItemUser`]).
///
/// Everything except a [`Piercing`] weapon resolves on the *first* creature in
/// the way and goes no further, whatever tile it was aimed at.
fn resolve_throw(world: &mut World, throw: WantsToThrow) {
    let WantsToThrow {
        thrower,
        item,
        target,
    } = throw;
    let Some(&origin) = world.get::<Position>(thrower) else {
        return;
    };

    // Gear leaves the hand the moment it is thrown, taking its bonuses with it.
    force_unequip(world, item);
    sync_equipment_effects(world, thrower);

    let seen_name = crate::identify::display_name(world, item);
    // Loosed from the launcher it's matched to (a bow's arrow, a crossbow's
    // quarrel), this reads as firing it, not just chucking it by hand.
    let fired = world
        .get::<LaunchedBy>(item)
        .is_some_and(|&LaunchedBy(launcher)| launcher.probe(world, thrower));
    let announcement = match (world.get::<Player>(thrower).is_some(), fired) {
        (true, true) => format!("You fire {}.", crate::identify::phrase_for(&seen_name)),
        (true, false) => format!("You throw the {seen_name}."),
        (false, true) => format!(
            "The {} fires {}.",
            item_label(world, thrower),
            crate::identify::phrase_for(&seen_name)
        ),
        (false, false) => format!("The {} throws the {seen_name}.", item_label(world, thrower)),
    };
    world.resource_mut::<GameLog>().add(announcement);

    let (cells, landing, victims) = flight_path(world, thrower, item, origin, target);
    let victim = victims.first().copied();
    if let Some((glyph, color)) = world.get::<Renderable>(item).map(|r| (r.glyph, r.color)) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hurl(&cells, glyph, color);
        }
    }

    // A potion is glass: it breaks on whatever it reaches, and whoever wears it
    // gets the dose.
    if let Some(effect) = world.get::<Potion>(item).map(|p| p.effect) {
        shatter_potion(world, item, victim, &seen_name, effect);
        return;
    }

    // A scroll only means something to a creature that can read it. Anything
    // else it bounces off, and it can be picked up again.
    if let Some(effect) = world.get::<Scroll>(item).map(|s| s.effect) {
        match victim.filter(|&v| world.get::<ItemUser>(v).is_some()) {
            Some(reader) => {
                let who = item_label(world, reader);
                world.resource_mut::<GameLog>().add(format!(
                    "The {who} unrolls the {seen_name} and reads it aloud!"
                ));
                apply_scroll_effect(world, reader, effect);
                // The words were spoken out loud, in front of you: whatever the
                // scroll was, it is no longer a mystery.
                identify_from_afar(world, item);
                world.entity_mut(item).despawn();
            }
            None => land_item(world, item, landing),
        }
        return;
    }

    // A wand is a stick with its magic held inside it. Hurl it instead of
    // zapping it and everything it was saving comes out at once, where it lands
    // — all its remaining charges spent in a single burst. It does not care
    // whose idea it was, so mind how close you are standing.
    //
    // But the glass only breaks on *impact*: the wand has to hit a creature or a
    // wall, or fly its full leash, before it goes off. Lobbed gently into open
    // floor, it just clatters down with its charges — and its secret — intact.
    if let Some(effect) = world.get::<Wand>(item).map(|w| w.effect) {
        let hit_creature = victim.is_some();
        let hit_wall = landing != target;
        let range_flown = (landing.x as i32 - origin.x as i32)
            .abs()
            .max((landing.y as i32 - origin.y as i32).abs());
        let spent_its_leash = range_flown >= THROW_RANGE;

        if hit_creature || hit_wall || spent_its_leash {
            resolve_wand_throw(world, item, landing, effect, &seen_name);
            identify_from_afar(world, item);
            world.entity_mut(item).despawn();
            return;
        }
        world.resource_mut::<GameLog>().add(format!(
            "The {seen_name} clatters to the floor, its magic still bottled up."
        ));
        land_item(world, item, landing);
        return;
    }

    // Everything else flies as a missile. A dagger or a spear spends itself on
    // everyone standing in the line; anything else has exactly one victim, or
    // none.
    let Some(victim) = victim else {
        land_item(world, item, landing);
        return;
    };

    for &hit in &victims {
        let msg = strike_victim(world, thrower, item, hit, landing, &seen_name);
        world.resource_mut::<GameLog>().add(msg);
    }

    // A missile built for the flight is spent on what it found: an arrow snaps,
    // a spear is left where it stuck. Nothing catches one, and there is nothing
    // left on the floor to collect.
    if world.get::<Projectile>(item).is_some() {
        world.entity_mut(item).despawn();
        return;
    }

    // A creature the throw just killed keeps nothing; the reaper will lay the
    // rest of its gear out beside this.
    let victim_name = item_label(world, victim);
    let slain = world.get::<Fighter>(victim).is_some_and(|f| f.hp <= 0);
    let takes_it =
        !slain && world.get::<ItemUser>(victim).is_some() && world.get::<Equipped>(item).is_some();
    if takes_it && equip_silently(world, victim, item) {
        let slot = world.get::<Equipped>(item).map(|e| e.slot);
        let verb = match slot {
            Some(Slot::Hand) => "snatches it up and wields it",
            Some(Slot::Body) => "pulls it on",
            _ => "slips it on",
        };
        world
            .resource_mut::<GameLog>()
            .add(format!("The {victim_name} {verb}!"));
        identify_from_afar(world, item);
        return;
    }
    land_item(world, item, landing);
}

/// A ring of alternating magenta / cyan sparks around `center` — the wand of
/// nothing's entire contribution to the dungeon.
fn confetti_burst(fx: &mut Particles, center: Position) {
    const RING: [(i32, i32); 9] = [
        (0, 0),
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, -1),
        (1, -1),
        (-1, 1),
    ];
    for (i, (dx, dy)) in RING.into_iter().enumerate() {
        let Some((x, y)) = crate::particles::on_map(center.x as i32 + dx, center.y as i32 + dy)
        else {
            continue;
        };
        let color = if i % 2 == 0 {
            Color::Magenta
        } else {
            Color::Cyan
        };
        fx.blip(x, y, '*', color);
    }
}

/// A thrown potion is glass: it breaks on the first thing it reaches and doses
/// whoever that was, or just wets the floor with nobody in the way. Either way
/// the vial is gone.
fn shatter_potion(
    world: &mut World,
    item: Entity,
    victim: Option<Entity>,
    seen_name: &str,
    effect: PotionEffect,
) {
    let Some(v) = victim else {
        world
            .resource_mut::<GameLog>()
            .add(format!("The {seen_name} shatters on the floor."));
        world.entity_mut(item).despawn();
        return;
    };
    let victim_name = item_label(world, v);
    world.resource_mut::<GameLog>().add(format!(
        "The {seen_name} bursts over the {victim_name}, which splutters and swallows a mouthful!"
    ));
    // A dose that plainly did something names the potion; a fizzle keeps its secret.
    if apply_potion_effect(world, v, effect) {
        identify_from_afar(world, item);
    }
    world.entity_mut(item).despawn();
}

/// One victim in a thrown missile's path: roll damage, apply it, spark the hit,
/// and return the line for the log.
fn strike_victim(
    world: &mut World,
    thrower: Entity,
    item: Entity,
    hit: Entity,
    landing: Position,
    seen_name: &str,
) -> String {
    let hit_name = item_label(world, hit);
    let Some(damage) = roll_throw_damage(world, thrower, item, hit) else {
        // Not a thing that hurts anyone: it simply arrives.
        return format!("The {seen_name} bounces off the {hit_name}.");
    };
    if damage <= 0 {
        // A weapon whose roll the armour ate.
        return format!("The {seen_name} glances off the {hit_name}.");
    }
    let at = world.get::<Position>(hit).copied().unwrap_or(landing);
    apply_damage(world, hit, damage);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.hit_spark(at.x, at.y);
    }
    format!("The {seen_name} hits the {hit_name} for {damage} damage.")
}
