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
use crate::identify::{article_for, counted, display_name, phrase_for, with_article};
use crate::map::{GameRng, Map};
use crate::particles::Particles;
use crate::shake::{ShakeKind, kick_shake};
use crate::traps::detonate_at;

use super::potions::apply_potion_effect;
use super::scrolls::apply_scroll_effect;
use super::wands::{
    blast_palette, cancel_entity, dazzle, elemental_blast, is_attack_wand, polymorph_entity,
    teleport_entity_away, teleport_entity_to_self,
};
use crate::conditions::shift_entity_speed;

use crate::constants::items::{LAUNCHER_RANGE, LIGHT_THROW_RANGE, PACK_CAPACITY, THROW_RANGE};
use crate::constants::wands::{
    BLAST_RADIUS, EFFECT_DIE_PER_CHARGE, GRENADE_DIE_PER_CHARGE, GRENADE_RADIUS,
};

/// Why `user` can't throw `item`, if they can't. Two things stay in the pack:
/// the Element of Yoord, which is the whole point of the run and is not to be
/// flung down a corridor, and cursed gear, which is welded on — it won't come
/// off, so it can be neither dropped nor hurled. Anything else is fair game.
pub fn throw_refusal(world: &World, user: Entity, item: Entity) -> Option<String> {
    if world.get::<Amulet>(item).is_some() {
        return Some(strings::element_wont_leave_hand().to_string());
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
    Some(equipped.slot.stuck(&display_name(world, item)))
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

/// Whether `item` is ammunition being *loosed* — it carries [`LaunchedBy`] and
/// `thrower` has the launcher effect it answers to — rather than chucked by
/// hand. The launcher entity is never consulted, only the effect, so a monster
/// that picked one up shoots exactly as the player does.
fn is_fired(world: &World, thrower: Entity, item: Entity) -> bool {
    world
        .get::<LaunchedBy>(item)
        .is_some_and(|&LaunchedBy(launcher)| launcher.probe(world, thrower))
}

/// A potion, a scroll, a wand or a ring — the small stuff, which carries
/// [`LIGHT_THROW_RANGE`] instead of the [`THROW_RANGE`] a spear gets.
fn is_light(world: &World, item: Entity) -> bool {
    world.get::<Potion>(item).is_some()
        || world.get::<Scroll>(item).is_some()
        || world.get::<Wand>(item).is_some()
        || world.get::<Ring>(item).is_some()
}

/// How far `thrower` can put `item`: [`LAUNCHER_RANGE`] when a launcher is
/// doing the work, [`LIGHT_THROW_RANGE`] for something small enough to flick,
/// an arm's [`THROW_RANGE`] otherwise. This is the reticle's leash, and the
/// only thing that bounds a shot — `flight_path` flies whatever line it is
/// given.
pub fn throw_reach(world: &World, thrower: Entity, item: Entity) -> i32 {
    if is_fired(world, thrower, item) {
        return LAUNCHER_RANGE;
    }
    match is_light(world, item) {
        true => LIGHT_THROW_RANGE,
        false => THROW_RANGE,
    }
}

/// What a hurled object does on impact, or `None` if it is not the sort of thing
/// that hurts anyone: only an item carrying [`ThrownDamage`] rolls at all, so a
/// wand or a suit of armour just bounces off and falls.
///
/// The roll is `1d[thrown damage]`, plus the item's own enchantment, plus every
/// [`ThrowBonus`] the *thrower* is wearing — a ring of sharpshooting, the plus on
/// the bow in their hand. Three things bend it:
///
/// * **A launcher switches the die.** A missile carrying [`LaunchedBy`] asks
///   whether its thrower has the effect it answers to; if so it rolls its
///   [`LaunchedDamage`] instead of [`ThrownDamage`] — an arrow lobbed by hand
///   rolls `1d4`, the same arrow loosed from a bow rolls `1d6`, a quarrel rolls
///   its plain double. The bow is not consulted — only the effect is, so a
///   monster that picked one up shoots just as well as you do.
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
    if is_fired(world, thrower, item) {
        die = world.get::<LaunchedDamage>(item).map_or(die, |d| d.0);
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

/// The first projectile in `holder`'s pack that answers to whatever launcher
/// effect they currently carry (`FireArrow` from a drawn bow, `FireQuarrel`
/// from a crossbow) — pack order, same as the letters the pack screen lists
/// them under. `None` if nothing in the pack matches, whether because the
/// pack has no ammunition at all or because it's the wrong kind for what's in
/// hand.
///
/// This is the shot `f` (fire) reaches for, and the one ranged auto-fight
/// looses.
pub fn first_matching_ammo(world: &World, holder: Entity) -> Option<Entity> {
    world
        .get::<Backpack>(holder)?
        .items
        .iter()
        .copied()
        .find(|&item| is_fired(world, holder, item))
}

/// "arrows" or "quarrels" — whichever a drawn launcher on `holder` calls for,
/// for a "you have no ___ to fire" refusal. Only meaningful once the caller
/// has already confirmed a launcher is wielded.
pub fn ammo_noun(world: &World, holder: Entity) -> &'static str {
    if world.get::<FireQuarrel>(holder).is_some() {
        "quarrels"
    } else {
        "arrows"
    }
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
        let label = with_article(world, item);
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
    Some(counted(&name, taking))
}

/// A thrown wand bursts where it lands, spending every charge it had left in one
/// go.
///
/// * An **attack** wand throws the wide, hot grenade —
///   [`GRENADE_DIE_PER_CHARGE`] sides a charge at [`GRENADE_RADIUS`],
///   armour-ignoring, elemental where the wand is.
/// * The **wand of light** throws the same wide grenade but [`dazzle`]s every
///   creature it catches instead of carrying an element.
/// * A **utility** wand throws a smaller [`BLAST_RADIUS`] blast that deals *no*
///   damage — the payload is the effect, worked on every creature caught.
/// * The **wand of nothing** just makes confetti.
fn resolve_wand_throw(
    world: &mut World,
    thrower: Entity,
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
        world
            .resource_mut::<GameLog>()
            .add(strings::thrown_wand_confetti(seen_name));
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

    world
        .resource_mut::<GameLog>()
        .add(strings::thrown_wand_shatters(seen_name, charges));

    let palette = blast_palette(effect);
    let caught = elemental_blast(
        world,
        Some(thrower),
        landing,
        radius,
        damage,
        element,
        palette,
    );

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
        match is_light {
            true => dazzle(world, entity),
            false => apply_thrown_wand_effect(world, entity, effect),
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
///
/// Assumes nothing upstream but a filled [`ThrowQueue`]: every throw was
/// already validated (range, a legal target) at the reticle before it was
/// queued, so this only has to resolve what arrives.
pub fn throw_system(world: &mut World) {
    let throws = std::mem::take(&mut world.resource_mut::<ThrowQueue>().throws);
    for throw in throws {
        // A thrown item is one of the things that lets go of a rapier's
        // built-up momentum — see `crate::equipment::reset_momentum`.
        crate::equipment::reset_momentum(world, throw.thrower);
        resolve_throw(world, throw);
    }
}

/// One thrown item, from the thrower's hand to wherever it comes to rest — and
/// then whatever is waiting on the tile it landed on.
///
/// A shot that comes down on a trap sets it off, and a trap with nobody
/// standing on it does not bite one victim: it bursts (see [`detonate_trap`]).
/// That is the trick shot, and it is why this is a wrapper rather than the
/// whole job — it happens whatever the item was, whether or not the shot hit
/// anyone, and whether or not it was the shot the thrower had in mind.
fn resolve_throw(world: &mut World, throw: WantsToThrow) {
    // Whether this was a shot or a lob has to be asked before the throw: a
    // potion that shatters is not around afterwards to be asked anything.
    let by_hand = world.get::<LaunchedBy>(throw.item).is_none();
    let thrower_is_player = world.get::<Player>(throw.thrower).is_some();

    let Some(landing) = deliver_throw(world, throw) else {
        return;
    };
    let Some(_shot) = detonate_at(world, landing, Some(throw.thrower)) else {
        return;
    };
    // Ammunition setting something off is what ammunition is for. A dagger, a
    // potion, somebody's spare ring — that is a choice, and the dungeon
    // notices.
    if by_hand && thrower_is_player {
        world.resource_mut::<GameLog>().add(strings::very_clever());
    }
}

/// The throw itself: from the thrower's hand to whatever it finds along its
/// line. A potion shatters over its target and is drunk by it; a scroll is read aloud
/// by anything literate enough and otherwise flutters to the floor; everything
/// else simply arrives — hurting what it hits only if it carries
/// [`ThrownDamage`], and staying with a creature that knows what to do with it
/// ([`ItemUser`]).
///
/// Everything except a [`Piercing`] weapon resolves on the *first* creature in
/// the way and goes no further, whatever tile it was aimed at.
///
/// Returns the tile the throw came down on, or `None` if there was no throw to
/// make.
fn deliver_throw(world: &mut World, throw: WantsToThrow) -> Option<Position> {
    let WantsToThrow {
        thrower,
        item,
        target,
    } = throw;
    let &origin = world.get::<Position>(thrower)?;

    // Gear leaves the hand the moment it is thrown, taking its bonuses with it.
    force_unequip(world, item);
    sync_equipment_effects(world, thrower);

    let seen_name = display_name(world, item);
    // Loosed from the launcher it's matched to (a bow's arrow, a crossbow's
    // quarrel), this reads as firing it, not just chucking it by hand.
    let fired = is_fired(world, thrower, item);
    let is_player = world.get::<Player>(thrower).is_some();
    let announcement = match (is_player, fired) {
        (true, true) => strings::you_fire(&phrase_for(&seen_name)),
        (true, false) => strings::you_throw(&seen_name),
        (false, true) => strings::mob_fires(&item_label(world, thrower), &phrase_for(&seen_name)),
        (false, false) => strings::mob_throws(&item_label(world, thrower), &seen_name),
    };
    let category = if is_player {
        LogCategory::Thrown
    } else {
        LogCategory::Plain
    };
    world
        .resource_mut::<GameLog>()
        .add_colored(announcement, category);

    let (cells, landing, victims) = flight_path(world, thrower, item, origin, target);
    let victim = victims.first().copied();
    // A thrown wand flies as a tumbling mystic grenade, tinted with the
    // element it's about to burst in — not its own catalog glyph, which is
    // what every other thrown item still flies as.
    match world.get::<Wand>(item).map(|w| w.effect) {
        Some(effect) => {
            let color = blast_palette(effect).accent_color();
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.lob(&cells, color);
            }
        }
        None => {
            if let Some((glyph, color)) = world.get::<Renderable>(item).map(|r| (r.glyph, r.color))
            {
                if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                    fx.hurl(&cells, glyph, color);
                }
            }
        }
    }

    // A potion is glass: it breaks on whatever it reaches, and whoever wears it
    // gets the dose.
    if let Some(effect) = world.get::<Potion>(item).map(|p| p.effect) {
        shatter_potion(world, item, victim, &seen_name, effect);
        return Some(landing);
    }

    // A scroll only means something to a creature that can read it. Anything
    // else it bounces off, and it can be picked up again.
    if let Some(effect) = world.get::<Scroll>(item).map(|s| s.effect) {
        match victim.filter(|&v| world.get::<ItemUser>(v).is_some()) {
            Some(reader) => {
                let who = item_label(world, reader);
                world
                    .resource_mut::<GameLog>()
                    .add(strings::scroll_read_aloud(&who, &seen_name));
                apply_scroll_effect(world, reader, effect);
                world.entity_mut(item).despawn();
            }
            None => land_item(world, item, landing),
        }
        return Some(landing);
    }

    // A wand is a stick with its magic held inside it. Hurl it instead of
    // zapping it and everything it was saving comes out at once, where it lands
    // — all its remaining charges spent in a single burst. It does not care
    // whose idea it was, so mind how close you are standing.
    //
    // But the glass only breaks on *impact*: the wand has to hit a creature or a
    // wall, or fly its full leash, before it goes off. Lobbed gently into open
    // floor, it just clatters down with its charges intact.
    if let Some(effect) = world.get::<Wand>(item).map(|w| w.effect) {
        let hit_creature = victim.is_some();
        let hit_wall = landing != target;
        let range_flown = (landing.x as i32 - origin.x as i32)
            .abs()
            .max((landing.y as i32 - origin.y as i32).abs());
        let spent_its_leash = range_flown >= throw_reach(world, thrower, item);

        if hit_creature || hit_wall || spent_its_leash {
            resolve_wand_throw(world, thrower, item, landing, effect, &seen_name);
            world.entity_mut(item).despawn();
            return Some(landing);
        }
        world
            .resource_mut::<GameLog>()
            .add(strings::wand_clatters_unspent(&seen_name));
        land_item(world, item, landing);
        return Some(landing);
    }

    // Everything else flies as a missile. A dagger or a spear spends itself on
    // everyone standing in the line; anything else has exactly one victim, or
    // none.
    let Some(victim) = victim else {
        land_item(world, item, landing);
        return Some(landing);
    };

    for &hit in &victims {
        // Aiming a shot at a medusa is a gaze like any other — see
        // `crate::abilities::medusa_gaze`.
        crate::abilities::fire_on_targeted(world, thrower, hit);
        let msg = strike_victim(world, thrower, item, hit, landing, &seen_name);
        world.resource_mut::<GameLog>().add(msg);
    }

    // A missile built for the flight is spent on what it found: an arrow snaps,
    // a spear is left where it stuck. Nothing catches one, and there is nothing
    // left on the floor to collect.
    if world.get::<Projectile>(item).is_some() {
        world.entity_mut(item).despawn();
        return Some(landing);
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
            Some(Slot::Hand) => strings::picked_up_thrown_verb_hand(),
            Some(Slot::Body) => strings::picked_up_thrown_verb_body(),
            _ => strings::picked_up_thrown_verb_other(),
        };
        world
            .resource_mut::<GameLog>()
            .add(strings::picks_up_thrown(&victim_name, verb));
        return Some(landing);
    }
    land_item(world, item, landing);
    Some(landing)
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

/// A launcher-wielding monster's shot: `shooter` looses at `target` exactly as
/// a fired missile always resolves — the ammunition's [`LaunchedDamage`] die,
/// the roll ignoring armour outright, plus whatever [`ThrowBonus`] the
/// launcher's own enchantment lends. A monster keeps no quiver to draw from,
/// so unlike the player's own shot this one never runs dry: [`crate::ai`]
/// calls it in place of a melee attack for as long as a launcher stays in its
/// hand.
///
/// A no-op if `shooter` isn't actually wielding one — the caller has already
/// checked, but this is the one place that knows how to loose a shot, so it
/// checks again rather than trust it.
pub(crate) fn monster_ranged_attack(world: &mut World, shooter: Entity, target: Entity) {
    if crate::equipment::wielded_launcher(world, shooter).is_none() {
        return;
    }
    let fires_quarrel = world.get::<FireQuarrel>(shooter).is_some();
    let (die, noun) = crate::catalog::ammo_launched_die(fires_quarrel);
    let bonus = equipped_total::<ThrowBonus>(world, shooter);
    let roll = world.resource_mut::<GameRng>().0.gen_range(1..=die) + bonus;
    let damage = roll.max(0);

    let shooter_name = item_label(world, shooter);
    let target_is_player = world.get::<Player>(target).is_some();
    let target_label = match target_is_player {
        true => "you".to_string(),
        false => strings::the(&item_label(world, target)),
    };
    let from = world.get::<Position>(shooter).copied();
    let at = world.get::<Position>(target).copied();
    if let (Some(from), Some(at)) = (from, at) {
        let cells: Vec<(u16, u16)> = get_line(from, at)
            .into_iter()
            .filter(|&p| p != from)
            .map(|p| (p.x, p.y))
            .collect();
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hurl(&cells, if fires_quarrel { '/' } else { '↑' }, Color::Grey);
        }
    }

    if damage <= 0 {
        world
            .resource_mut::<GameLog>()
            .add(strings::monster_shot_wild(&shooter_name, noun, &target_label));
        return;
    }

    apply_damage(world, target, damage);
    if let Some(at) = at {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hit_spark(at.x, at.y);
        }
    }
    world.resource_mut::<GameLog>().add(strings::monster_shot_hit(
        &shooter_name,
        article_for(noun),
        noun,
        &target_label,
        damage,
    ));
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
            .add(strings::potion_shatters_floor(seen_name));
        world.entity_mut(item).despawn();
        return;
    };
    let victim_name = item_label(world, v);
    world
        .resource_mut::<GameLog>()
        .add(strings::potion_bursts_over(seen_name, &victim_name));
    apply_potion_effect(world, v, effect);
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
        return strings::throw_bounces_off(seen_name, &hit_name);
    };
    let at = world.get::<Position>(hit).copied().unwrap_or(landing);
    if damage <= 0 {
        // A weapon whose roll the armour ate — melee's glancing blow, at
        // range, and it reads the same way: the cold clink and no shake.
        // Harsher than melee, which has a chip-damage floor under it; a
        // missile that can't beat armour does nothing at all.
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.clink_spark(at.x, at.y);
        }
        return strings::throw_glances_off(seen_name, &hit_name);
    }
    apply_damage(world, hit, damage);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.hit_spark(at.x, at.y);
    }
    // A shot of the player's that drew blood thumps exactly as much as their
    // sword landing would. Two exclusions, the same two melee has: a monster's
    // shot never shakes, and a kill takes its own kick from the reaper — which
    // is also the sight gate for one, since `apply_damage` leaves a lethal
    // missile hit for `finish_indirect_kill` to finalise.
    let slain = world.get::<Fighter>(hit).is_some_and(|f| f.hp <= 0);
    if world.get::<Player>(thrower).is_some() && !slain {
        kick_shake(world, ShakeKind::Hit);
    }
    strings::throw_hits(seen_name, &hit_name, damage)
}
