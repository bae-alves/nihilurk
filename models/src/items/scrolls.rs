//! Reading a scroll.
//!
//! The catalog ([`crate::catalog::SCROLLS`]) names each scroll; this file is
//! where the words on it take effect, keyed by [`ScrollEffect`]. A scroll read
//! aloud by a *monster* you threw it at runs the same [`apply_scroll_effect`],
//! with the monster as the reader (see [`super::throwing`]).

use bevy_ecs::{entity::Entity, prelude::With, world::World};
use crossterm::style::Color;
use rand::Rng;

use crate::components::*;
use crate::conditions::{confuse, snare};
use crate::constants::scrolls::*;
use crate::effects::{ArmorBonus, ArmorDie, PowerBonus, PowerDie, ThrowBonus};
use crate::equipment::{Slot, equipped_in, equipped_items, force_unequip, sync_equipment_effects};
use crate::helpers::{
    actor_line, free_adjacent_tile, hostiles_in_view, item_label, mark_conditions, spark_burst_at,
    tile_of,
};
use crate::identify::{Identified, article_for};
use crate::magicmap::{MagicMapReveal, MagicMapStyle};
use crate::map::{GameRng, MAP_HEIGHT, MAP_WIDTH};
use crate::monsters::{BESTIARY, spawn_monster};
use crate::particles::Particles;
use crate::traps::random_open_tile;

use super::potions::{is_the_relic, worth_detecting};

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

/// Exhaustive over `ScrollEffect`, deliberately with no catch-all: a scroll
/// effect added to the enum and not given an arm here fails the build instead
/// of reading as a generic "nothing obvious happens" — the same guarantee
/// `crate::traps::apply_trap_effect` gives a new `TrapEffect`. See
/// `docs/explanation/data-driven-content.md`.
///
/// Every row in the table now does something; blank paper is the only scroll
/// that reads as nothing happening, and it means it.
pub(super) fn apply_scroll_effect(world: &mut World, user: Entity, effect: ScrollEffect) {
    match effect {
        ScrollEffect::Identify => identify_random_unknown_item(world, user),
        ScrollEffect::RemoveCurse => {
            let freed = lift_curses(world, user);
            let msg = if freed > 0 {
                "You feel as though somebody is watching over you. Your cursed gear crumbles away."
            } else {
                "You feel as though somebody is watching over you."
            };
            world.resource_mut::<GameLog>().add(msg.to_string());
        }
        ScrollEffect::MagicMapping => {
            // Roll the wipe's shape (or take the `ROOG_MAGICMAP` dev override),
            // then arm it centred on the reader. The engine plays it out frame
            // by frame after the turn (see [`crate::magicmap`]); headless
            // callers with no reveal resource just skip the animation.
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
        }
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
        ScrollEffect::EnchantWeapon => enchant_gear(world, user, Slot::Hand),
        ScrollEffect::EnchantArmor => enchant_gear(world, user, Slot::Body),
        ScrollEffect::MonsterConfusion => charm_hands(world, user),
        ScrollEffect::HoldMonster => hold_in_view(world, user),
        ScrollEffect::Sleep => read_sleep(world, user),
        ScrollEffect::FoodDetection => detect_mundane_items(world, user),
    }
}

/// Scroll of teleportation: whisk the reader to a random open tile somewhere on
/// the current floor.
pub(super) fn teleport_reader(world: &mut World, user: Entity) {
    // The magenta puff a teleport always leaves where its victim stood — the
    // wand of teleportation's calling card, and the teleport trap's.
    let was = world.get::<Position>(user).copied();
    if let Some(was) = was {
        crate::helpers::leave_tinted_smoke(world, was, Color::Magenta);
    }
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut pos) = world.get_mut::<Position>(user) {
            pos.x = x;
            pos.y = y;
        }
        if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
            vs.dirty = true;
        }
    }
    // A teleport carries you clean out of whatever was holding you in place —
    // otherwise a bear trap's Snare survives the jump and keeps thrashing a
    // leg on a tile nowhere near the actual trap.
    world.entity_mut(user).remove::<crate::components::Snare>();
    world
        .resource_mut::<GameLog>()
        .add("BLONK! You are whisked away!".to_string());
}

/// Scroll of aggravate monsters: every creature on the floor drops what it was
/// doing and homes in on the reader's tile — in or out of sight. See
/// [`MovementType::Aggravated`].
fn aggravate_floor(world: &mut World, user: Entity) {
    let _heard = aggravate_all_monsters(world, user);
    world
        .resource_mut::<GameLog>()
        .add("A shrill shriek rips through the dungeon. Everything on this floor heard it — and it knows where you are.".to_string());
}

/// The bare mechanic: point every hostile on the floor at `origin`'s tile. The
/// scroll of aggravate monsters wraps this in its own flavour; so does the
/// [`crate::effects::AggravatesMonsters`] passive (see [`crate::abilities`]).
///
/// Returns whether anything heard it. An empty floor makes no noise, which is
/// what keeps a cursed ring from announcing itself in a dungeon with nothing
/// left alive on it.
pub(crate) fn aggravate_all_monsters(world: &mut World, origin: Entity) -> bool {
    let Some(&hero) = world.get::<Position>(origin) else {
        return false;
    };
    let mobs: Vec<Entity> = world
        .query_filtered::<Entity, With<Mob>>()
        .iter(world)
        .collect();
    let mut heard = false;
    for m in mobs {
        if world.get::<Faction>(m) != Some(&Faction::Monster) {
            continue;
        }
        if let Some(mut mob) = world.get_mut::<Mob>(m) {
            mob.movement_type = MovementType::Aggravated {
                tx: hero.x,
                ty: hero.y,
            };
            heard = true;
        }
    }
    heard
}

/// Scroll of scare monster: every monster currently in the reader's view turns
/// tail for good. Returns how many were scared.
fn scare_in_view(world: &mut World, user: Entity) -> usize {
    let targets = hostiles_in_view(world, user);
    for t in &targets {
        if let Some(mut mob) = world.get_mut::<Mob>(*t) {
            mob.movement_type = MovementType::Flee;
        }
    }
    mark_conditions(world, &targets, '!', Color::Yellow);
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
        article_for(&name)
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

// ---------------------------------------------------------------------------
// Enchantment
// ---------------------------------------------------------------------------

/// What a plus becomes when a scroll of enchantment is read over it: one better,
/// unless it was *negative*, in which case the whole minus is mended at once.
///
/// A cursed -5 sword is not a project. One scroll makes it an honest +0 blade,
/// and — since the same reading burns the [`Curse`] off — one you can finally
/// take out of your hand.
fn mended_plus(bonus: i32) -> i32 {
    if bonus < 0 {
        return 0;
    }
    bonus + ENCHANT_BONUS
}

/// Adds a point to whichever flat modifier `item` wears its plus in. Which one
/// that is, is read off the item exactly the way `catalog::enchant_equipment`
/// reads it when the dungeon rolls a drop: a thing with a [`PowerDie`] is a
/// weapon, a thing with an [`ArmorDie`] is armour, a [`Launcher`] is a bow whose
/// enchantment has no melee roll to land on and so lands on the throw. Returns
/// whether there was anything on it to improve.
fn raise_plus(world: &mut World, item: Entity) -> bool {
    let mut e = world.entity_mut(item);
    let mut raised = false;
    if e.contains::<PowerDie>() {
        let base = e.get::<PowerBonus>().map_or(0, |b| b.0);
        e.insert(PowerBonus(mended_plus(base)));
        raised = true;
    }
    if e.contains::<ArmorDie>() {
        let base = e.get::<ArmorBonus>().map_or(0, |b| b.0);
        e.insert(ArmorBonus(mended_plus(base)));
        raised = true;
    }
    if e.contains::<Launcher>() {
        let base = e.get::<ThrowBonus>().map_or(0, |b| b.0);
        e.insert(ThrowBonus(mended_plus(base)));
        raised = true;
    }
    raised
}

/// Scroll of enchant weapon / enchant armor: the gear in the reader's `slot`
/// gains a permanent point of plus ([`mended_plus`]) and loses its curse, in a
/// shower of orange sparks. Reading it also settles what the item *is* — you
/// watched the enchantment take, so there is nothing left to be coy about
/// ([`KnownQuality`]).
///
/// Read over an empty hand (or an unarmoured back) it gutters out the way a
/// scroll of vorpalize weapon does: the words need something to bite into.
fn enchant_gear(world: &mut World, user: Entity, slot: Slot) {
    if enchant_equipped(world, user, slot) {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(missing_gear_line(slot).to_string());
}

/// The enchantment itself, with no word for the case where there was nothing to
/// enchant: a point of plus onto whatever `user` has in `slot`, the curse burnt
/// off with it, and the item's quality settled. Returns whether it landed.
///
/// Split out from [`enchant_gear`] because a forge coin
/// ([`crate::items::pickups`]) buys the same enchantment but has two slots to
/// try and wants to move on to the second when the first is empty, rather than
/// telling the player about bare skin.
pub(crate) fn enchant_equipped(world: &mut World, user: Entity, slot: Slot) -> bool {
    let Some(item) = equipped_in(world, user, slot) else {
        return false;
    };
    let was_cursed = world.get::<Curse>(item).is_some();
    if !raise_plus(world, item) {
        return false;
    }
    world.entity_mut(item).remove::<Curse>();
    world.entity_mut(item).insert(KnownQuality);

    let name = item_label(world, item);
    spark_burst_at(world, user, Color::DarkYellow);
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} throws off a shower of orange sparks!"));
    if !was_cursed {
        return true;
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The curse on the {name} burns away with them."));
    true
}

/// Why an enchantment found nothing to land on — an empty hand for the weapon
/// scroll, bare skin for the armour one.
fn missing_gear_line(slot: Slot) -> &'static str {
    match slot {
        Slot::Body => "The sparks flare over bare skin and die. You are wearing no armour.",
        _ => "The sparks flare over an empty hand and die.",
    }
}

// ---------------------------------------------------------------------------
// Monster confusion
// ---------------------------------------------------------------------------

/// Scroll of monster confusion: instead of going off now, the charm settles into
/// the reader's hands and waits ([`ConfusingTouch`]). The next blow they *land*
/// spends it, and whatever they hit reels — see
/// [`crate::combat::resolve_attack`], which is where the charge is discharged.
///
/// Reading a second one before spending the first is not wasted so much as
/// redundant: the hands can only be charged, not more charged.
fn charm_hands(world: &mut World, user: Entity) {
    let already = world.get::<ConfusingTouch>(user).is_some();
    world.entity_mut(user).insert(ConfusingTouch);
    spark_burst_at(world, user, Color::Magenta);
    let fresh = actor_line(
        world,
        user,
        "Your hands begin to glow with a violet light. The next thing you touch will regret it.",
        "flexes its claws, and a violet light crawls over them",
    );
    let deeper = actor_line(
        world,
        user,
        "The violet light on your hands deepens. It is still one touch.",
        "shakes out its glowing claws",
    );
    let msg = match already {
        true => deeper,
        false => fresh,
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Spends a charged [`ConfusingTouch`] on whatever `attacker` just landed a blow
/// on: the charm passes into the victim and the hands go dark again. A no-op for
/// an attacker whose hands were never charged, which is every attacker in the
/// dungeon that has not read the scroll.
///
/// Lives here rather than in `crate::combat` because *what the charm does* is
/// the scroll's business; combat only knows when a blow landed.
pub(crate) fn discharge_confusing_touch(world: &mut World, attacker: Entity, victim: Entity) {
    if world.get::<ConfusingTouch>(attacker).is_none() {
        return;
    }
    world.entity_mut(attacker).remove::<ConfusingTouch>();
    spark_burst_at(world, victim, Color::Magenta);
    confuse(
        world,
        victim,
        "The violet light bursts against you — the room tilts!",
        "staggers as the violet light bursts over it",
    );
}

// ---------------------------------------------------------------------------
// Hold monster
// ---------------------------------------------------------------------------

/// Scroll of hold monster: everything in sight is rooted where it stands for
/// [`HOLD_TURNS`] turns ([`SnareKind::Hold`]). A held monster is pinned, not
/// helpless — walk into its reach and it still bites — so what the scroll buys
/// is the room to leave, or the range to shoot from.
fn hold_in_view(world: &mut World, user: Entity) {
    let caught = hostiles_in_view(world, user);
    for &t in &caught {
        snare(world, t, SnareKind::Hold, HOLD_TURNS);
    }
    mark_conditions(world, &caught, '#', Color::Cyan);
    let msg = match caught.is_empty() {
        true => "The words land like iron — on nothing at all.",
        false => "The words land like iron. Every creature in sight is rooted where it stands.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

// ---------------------------------------------------------------------------
// Sleep
// ---------------------------------------------------------------------------

/// Scroll of sleep: a drowsiness that rolls out over everything in sight and
/// drops it for [`SLEEP_TURNS`] turns — most of the time. On a
/// [`SLEEP_BACKFIRE_CHANCE`] roll the words turn in the reader's mouth and put
/// *them* out instead, which is the whole character of the scroll: a panic
/// button that is occasionally the emergency.
///
/// A sleeper is helpless, not merely pinned — no action of any kind until it
/// wears off — so this is the one scroll that hands out free kills, in either
/// direction.
fn read_sleep(world: &mut World, user: Entity) {
    let backfires = world
        .resource_mut::<GameRng>()
        .0
        .gen_bool(SLEEP_BACKFIRE_CHANCE);
    if backfires {
        sleep_the_reader(world, user);
        return;
    }
    let caught = hostiles_in_view(world, user);
    for &t in &caught {
        snare(world, t, SnareKind::Sleep, SLEEP_TURNS);
    }
    mark_conditions(world, &caught, 'z', Color::Blue);
    let msg = match caught.is_empty() {
        true => "A wave of drowsiness rolls out over an empty room.",
        false => "A wave of drowsiness rolls out, and everything in sight goes down with it.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

/// The backfire: the reader reads themselves to sleep.
fn sleep_the_reader(world: &mut World, user: Entity) {
    snare(world, user, SnareKind::Sleep, SLEEP_TURNS);
    if let Some((x, y)) = tile_of(world, user) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.condition_mark(x, y, 'z', Color::Blue, 0.0);
        }
    }
    let msg = actor_line(
        world,
        user,
        "The words slur and thicken in your own mouth. The floor comes up to meet you...",
        "reads itself into a heap on the floor",
    );
    world.resource_mut::<GameLog>().add(msg);
}

// ---------------------------------------------------------------------------
// Food detection
// ---------------------------------------------------------------------------

/// Scroll of food detection: the mundane half of a floor's contents —
/// everything a potion of magic detection would sniff past as beneath its notice
/// — is [`Detected`] where it lies, drawn dimly until the reader leaves the
/// floor.
///
/// The two detections divide the floor exactly between them
/// ([`worth_detecting`] is the line), with one deliberate exception: the Element
/// of Yoord shows up for both. It is the run. No sense that reaches across a
/// floor is going to miss it on a technicality about what counts as magic.
///
/// Like the potions, it is only worth anything to the player: a monster that
/// reads one aloud has learned where the coins are and no way to care.
fn detect_mundane_items(world: &mut World, user: Entity) {
    if world.get::<Player>(user).is_none() {
        world
            .resource_mut::<GameLog>()
            .add("The words mean nothing to it.".to_string());
        return;
    }
    let loose: Vec<Entity> = world
        .query_filtered::<Entity, (With<Item>, With<Position>)>()
        .iter(world)
        .collect();
    let found: Vec<Entity> = loose
        .into_iter()
        .filter(|&e| is_the_relic(world, e) || !worth_detecting(world, e))
        .collect();
    for item in &found {
        world.entity_mut(*item).insert(Detected);
    }
    spark_burst_at(world, user, Color::Green);
    let msg = match found.is_empty() {
        true => "You cast about for anything plain and useful, and this floor is bare.",
        false => "The floor gives up its odds and ends: you know where every plain thing lies.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}
