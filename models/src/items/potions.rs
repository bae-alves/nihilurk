//! Quaffing a potion.
//!
//! The catalog ([`crate::catalog::POTIONS`]) says what each potion is called and
//! how it draws; this file is the one place a dose actually *does* something,
//! keyed by [`PotionEffect`].
//!
//! Three shapes recur, and they are worth knowing before reading the match:
//!
//! * **A permanent change to the drinker's body** — healing, gain strength,
//!   poison, restore strength. These reach straight into [`Fighter`] and stay
//!   there; no staircase gives them back.
//! * **A condition** — blindness, confusion, paralysis, haste. These are the
//!   verbs in [`crate::conditions`], which knows the difference between the
//!   player (a marker component the input loop reads) and a monster (a random
//!   walk, or a slower tempo). They ride along until a staircase or a wand of
//!   cancellation lifts them.
//! * **Something that only means anything to the player** — the two detections,
//!   see invisible, raise level. A monster that swallows one of these is simply
//!   a monster that swallowed a mouthful of glass: the dose reports that nothing
//!   visible happened, and the potion keeps its secret.

use bevy_ecs::{entity::Entity, prelude::With, world::World};

use crate::components::*;
use crate::conditions::{blind, confuse, hasten, paralyse};
use crate::constants::potions::*;
use crate::effects::{
    ArmorBonus, Detected, Grant, Lifetime, PowerBonus, SeesInvisible, ThrowBonus, grant_for_floor,
};
use crate::helpers::actor_line;
use crate::map::{LevelChange, holding_element_of_yoord, transition_level};

/// Works a potion on `user`. Returns whether the dose visibly took hold — the
/// player learns a potion by drinking it either way, but a potion *thrown* at a
/// monster only gives itself away when something plainly happens (see
/// [`super::throwing`]).
///
/// Exhaustive over `PotionEffect`, deliberately with no catch-all: a potion
/// effect added to the enum and not given an arm here fails the build instead
/// of silently doing nothing — the same guarantee `crate::traps::apply_trap_effect`
/// gives a new `TrapEffect`. See `docs/explanation/data-driven-content.md`.
pub(super) fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) -> bool {
    match effect {
        PotionEffect::Healing => heal_fully(
            world,
            user,
            HEALING_MAX_HP_GAIN,
            "You feel refreshed as your wounds mend!",
        ),
        PotionEffect::ExtraHealing => heal_fully(
            world,
            user,
            EXTRA_HEALING_MAX_HP_GAIN,
            "You have never felt better than this!",
        ),
        PotionEffect::Blindness => blind(world, user),
        PotionEffect::Confusion => confuse(
            world,
            user,
            "The world is spinning! you are confused!",
            "reels, eyes swimming",
        ),
        PotionEffect::Paralysis => paralyse(world, user),
        PotionEffect::Haste => hasten(world, user),
        PotionEffect::GainStrength => gain_strength(world, user),
        PotionEffect::Poison => poison(world, user),
        PotionEffect::RestoreStrength => restore_strength(world, user),
        PotionEffect::SeeInvisible => see_invisible(world, user),
        PotionEffect::MagicDetection => detect_magic(world, user),
        PotionEffect::MonsterDetection => detect_monsters(world, user),
        PotionEffect::RaiseLevel => raise_level(world, user),
        PotionEffect::FruitJuice => flavour(world, user, "Cold, sweet and thick. Yummy!"),
        PotionEffect::Water => flavour(world, user, "It is water. Just water."),
    }
}

// ---------------------------------------------------------------------------
// The body
// ---------------------------------------------------------------------------

/// Healing and extra healing: fills the drinker back up and raises their
/// ceiling by `max_hp_gain` for good. Raising the ceiling is the point as much
/// as the refill is — nothing else in the dungeon grows the hero's HP pool, so
/// a potion of healing drunk at full health is not wasted.
fn heal_fully(world: &mut World, user: Entity, max_hp_gain: i32, player_line: &str) -> bool {
    let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
        return false;
    };
    let before = fighter.hp;
    fighter.max_hp += max_hp_gain;
    fighter.hp = fighter.max_hp;
    let healed = fighter.hp > before;
    let msg = actor_line(world, user, player_line, "glows eerily, wounds closing");
    world.resource_mut::<GameLog>().add(msg);
    healed
}

/// Potion of gain strength: a point of attack die, floor and ceiling both, so a
/// later potion of restore strength restores the *new* peak rather than the old
/// one.
fn gain_strength(world: &mut World, user: Entity) -> bool {
    let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
        return false;
    };
    fighter.power += GAIN_STRENGTH_POWER;
    fighter.max_power += GAIN_STRENGTH_POWER;
    let msg = actor_line(
        world,
        user,
        "You feel stronger. What bulging muscles!",
        "swells with muscle",
    );
    world.resource_mut::<GameLog>().add(msg);
    true
}

/// Potion of poison: takes the strength back out of the drinker's arm and does
/// not give it back. [`POISON_POWER_FLOOR`] keeps them armed, barely; a potion
/// of restore strength is the only cure.
fn poison(world: &mut World, user: Entity) -> bool {
    let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
        return false;
    };
    let before = fighter.power;
    fighter.power = (fighter.power - POISON_POWER_LOSS).max(POISON_POWER_FLOOR);
    let sickened = fighter.power < before;
    let msg = actor_line(
        world,
        user,
        "You feel very sick now — the strength drains out of you.",
        "retches, their limbs going slack",
    );
    world.resource_mut::<GameLog>().add(msg);
    sickened
}

/// Potion of restore strength: back up to [`Fighter::max_power`], undoing every
/// poisoned dart and every potion of poison at once.
fn restore_strength(world: &mut World, user: Entity) -> bool {
    let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
        return false;
    };
    let before = fighter.power;
    fighter.power = fighter.max_power;
    let restored = fighter.power > before;
    if !restored {
        let msg = actor_line(world, user, "You feel warm all over.", "shivers");
        world.resource_mut::<GameLog>().add(msg);
        return false;
    }
    let msg = actor_line(
        world,
        user,
        "Your old strength comes surging back into your arm.",
        "straightens, their strength returning",
    );
    world.resource_mut::<GameLog>().add(msg);
    true
}

// ---------------------------------------------------------------------------
// The senses
// ---------------------------------------------------------------------------

/// Potion of see invisible: the ring of perception's sight, lent for the floor
/// only ([`crate::effects::Lifetime::Floor`]). Like the ring it is retroactive —
/// the phantom stalking you and the invisible stash two rooms over both turn up
/// the moment the visibility system next runs.
fn see_invisible(world: &mut World, user: Entity) -> bool {
    let already = world.get::<SeesInvisible>(user).is_some();
    grant_for_floor(world, user, Grant::of::<SeesInvisible>());
    if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
        vs.dirty = true;
    }
    let msg = actor_line(
        world,
        user,
        "Your eyes sting, and the air fills with things that were never not there.",
        "eyes gleam, tracking something unseen",
    );
    world.resource_mut::<GameLog>().add(msg);
    !already
}

/// Potion of monster detection: every creature on the floor is [`Detected`] —
/// drawn dimly wherever it stands until the player leaves the floor. They are
/// sensed, not watched: the glyphs mark positions at the moment of drinking and
/// no sighting is announced.
fn detect_monsters(world: &mut World, user: Entity) -> bool {
    if world.get::<Player>(user).is_none() {
        return false;
    }
    let mobs: Vec<Entity> = world
        .query_filtered::<Entity, With<Mob>>()
        .iter(world)
        .collect();
    for mob in &mobs {
        crate::effects::lend(world, *mob, Grant::of::<Detected>(), Lifetime::Floor);
    }
    let msg = match mobs.is_empty() {
        true => "You listen hard, and hear nothing at all moving on this floor.",
        false => "You feel the floor's inhabitants shifting in the dark.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
    !mobs.is_empty()
}

/// Potion of magic detection: every *magic* item lying on the floor is
/// [`Detected`] — and turned up if it was stashed ([`detect_item`]). Anything
/// with a type-key — potion, scroll, wand, ring — qualifies; a weapon or a suit
/// of armour only registers once there is something magic about it, which is
/// the thing worth knowing in advance ([`worth_detecting`]).
fn detect_magic(world: &mut World, user: Entity) -> bool {
    if world.get::<Player>(user).is_none() {
        return false;
    }
    let loose: Vec<Entity> = world
        .query_filtered::<Entity, (With<Item>, With<Position>)>()
        .iter(world)
        .collect();
    let found: Vec<Entity> = loose
        .into_iter()
        .filter(|&e| worth_detecting(world, e))
        .collect();
    for item in &found {
        detect_item(world, *item);
    }
    let msg = match found.is_empty() {
        true => "You reach for the hum of magic, and this floor holds none.",
        false => "Magic hums up through the floor, and you know where every piece of it lies.",
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
    !found.is_empty()
}

/// Whether a potion of magic detection picks `item` out. A type-key is magic by
/// definition. Gear is not: a plain sword is metal, and only an enchantment
/// (either way — a curse is magic too, and being warned about one is half the
/// value of the potion) puts it on the map. The Element of Yoord answers to
/// every sense there is ([`is_the_relic`]).
///
/// Also the *dividing line* between the two detections: a scroll of food
/// detection turns up precisely what this rejects (see
/// [`super::scrolls::detect_mundane_items`]), so between them they find
/// everything on the floor exactly once.
pub(super) fn worth_detecting(world: &World, item: Entity) -> bool {
    let keyed = world.get::<Potion>(item).is_some()
        || world.get::<Scroll>(item).is_some()
        || world.get::<Wand>(item).is_some()
        || world.get::<Ring>(item).is_some();
    let enchanted = world.get::<PowerBonus>(item).is_some_and(|b| b.0 != 0)
        || world.get::<ArmorBonus>(item).is_some_and(|b| b.0 != 0)
        || world.get::<ThrowBonus>(item).is_some_and(|b| b.0 != 0);
    keyed || enchanted || world.get::<Curse>(item).is_some() || is_the_relic(world, item)
}

/// Mark one floor item as [`Detected`] until the player leaves the floor, and
/// turn it up if it was stashed. Both item detections — the potion's magic and
/// the scroll's mundane — go through here.
///
/// The reveal is not a bonus, it is what makes the mark mean anything. The
/// `Detected` render pass paints only tiles the player *cannot* see, so a stash
/// left wearing [`Hidden`] would glow from across the floor and then wink out
/// the moment the player walked into the room. A ring of perception already
/// turns a stash up for good ([`crate::visibility`]); a sense that reached the
/// whole floor doing less than that would be the strange one.
pub(super) fn detect_item(world: &mut World, item: Entity) {
    crate::effects::lend(world, item, Grant::of::<Detected>(), Lifetime::Floor);
    world.entity_mut(item).remove::<Hidden>();
    world.entity_mut(item).remove::<Invisible>();
}

/// Whether `item` is the Element of Yoord. The one thing in the dungeon that
/// both detections find: it is the run, and no sense that reaches across a
/// floor is going to miss it on a technicality about what counts as magic.
pub(super) fn is_the_relic(world: &World, item: Entity) -> bool {
    world.get::<Amulet>(item).is_some()
}

// ---------------------------------------------------------------------------
// The stairs you didn't climb
// ---------------------------------------------------------------------------

/// Potion of raise level: the drinker is pulled up one floor whether or not they
/// carry the Element of Yoord — the one thing in the dungeon that ignores the
/// Dungeon Lord's hold on the up-stair.
///
/// On Depth 1 there is no floor above, and what happens then turns on the relic.
/// Carrying it, the potion takes the last step for you and the run is won — an
/// alternate victory, and the reason a potion of raise level is worth hoarding
/// rather than drinking to find out what it is. Without it there is nothing up
/// there to rise *to*, and the dungeon finds that funny.
fn raise_level(world: &mut World, user: Entity) -> bool {
    if world.get::<Player>(user).is_none() {
        return false;
    }
    if world.resource::<Depth>().what > 1 {
        transition_level(world, false, LevelChange::Potion);
        return true;
    }
    if !holding_element_of_yoord(world) {
        world
            .resource_mut::<GameLog>()
            .add("You hear distant laughter.".to_string());
        return false;
    }
    world.resource_mut::<GameLog>().add(
        "The potion hauls you up through stone and root and out into the open sky. You are free."
            .to_string(),
    );
    crate::map::win_with_style(world);
    true
}

// ---------------------------------------------------------------------------
// The ones that are only a taste
// ---------------------------------------------------------------------------

/// Fruit juice and plain water: a line and nothing else. They report *no* effect
/// on purpose, so a monster you lob one at never tells you which of the two it
/// was — and neither does a floor-full of unidentified vials.
fn flavour(world: &mut World, user: Entity, player_line: &str) -> bool {
    let msg = actor_line(world, user, player_line, "smacks their lips");
    world.resource_mut::<GameLog>().add(msg);
    false
}
