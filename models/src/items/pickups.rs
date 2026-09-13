//! Coins, and what stepping on one does.
//!
//! A pickup is the one item the pack never sees: it works where it lies and is
//! gone. That single rule is what the whole module is about —
//!
//! * [`pick_up`] is the *only* way anything leaves the floor, pickup or not, so
//!   there is one place that knows a coin is answered differently from a sword.
//! * [`would_help`] is asked before a coin is spent, and a coin that would do
//!   nothing is simply not taken. Walking over a red coin at full health leaves
//!   it there for the fight that goes badly, and auto-explore
//!   ([`crate::autoexplore`]) reads the same question before it detours.
//! * Two of them do not pay out now at all. The platinum and forge coins leave
//!   a [`Plated`] / [`Forged`] promise on you, and the staircase settles it
//!   ([`settle_promises`]) — if you got there unhurt.
//!
//! The mechanic is an exhaustive match on [`PickupEffect`] with no catch-all, so
//! a new coin does not build until it does something. What it does *with* — how
//! many hit points, how many afflictions — is the `amount` on its
//! [`crate::catalog::CoinDef`] row, never a number in this file.

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::equipment::Slot;
use crate::identify::display_name;
use crate::map::GameRng;
use crate::score::award;

// ---------------------------------------------------------------------------
// Taking things off the floor
// ---------------------------------------------------------------------------

/// Everything that happens when `taker` steps onto `item`: the invisible-stash
/// reveal, the score a treasure carries, a pickup's effect, or an ordinary item
/// going into the pack. Returns the line to log, or `None` when the item is
/// left exactly where it was.
///
/// The one door onto the floor, so the rules about what can and cannot be
/// carried live together instead of in the input handler.
pub fn pick_up(world: &mut World, taker: Entity, item: Entity) -> Option<String> {
    // An invisibly-stashed item announces itself the instant you blunder onto
    // its tile, and is then treated like anything else.
    if world.get::<Hidden>(item).is_some() {
        world.entity_mut(item).remove::<Hidden>();
        world.entity_mut(item).remove::<Invisible>();
        world
            .resource_mut::<GameLog>()
            .add("Hey! There's something here!".to_string());
    }

    if world.get::<Pickup>(item).is_some() {
        return spend_pickup(world, taker, item);
    }

    // Score first: the relic is worth its 25000 the moment it is in hand, and a
    // pack too full to take it is a different sentence, not a different payment.
    let taken = crate::items::stow(world, taker, item)?;
    pay_out_value(world, item);
    let line = match world.get::<Amulet>(item).is_some() {
        true => {
            "You take the Element of Yoord. \"The element of Yoord seeks the sun.\"".to_string()
        }
        false => format!("You pick up {taken}."),
    };
    Some(line)
}

/// Pays whatever score an item carries into the run's total as it is taken. A
/// thing with no [`Value`] is worth nothing and says nothing.
///
/// **Once.** The [`Value`] comes off with the payment, so the one item that can
/// be paid for and then set down again — the Element of Yoord — cannot be
/// dropped and re-taken for another 25000. A coin never needed the rule; it is
/// spent the moment it is stepped on.
fn pay_out_value(world: &mut World, item: Entity) {
    let Some(amount) = world.get::<Value>(item).map(|v| v.amount) else {
        return;
    };
    world.entity_mut(item).remove::<Value>();
    award(world, amount);
}

/// Works one coin and destroys it, or leaves it on the floor when it would do
/// nothing at all. `None` back means the coin is still lying there.
fn spend_pickup(world: &mut World, taker: Entity, item: Entity) -> Option<String> {
    let pickup = world.get::<Pickup>(item).map(|p| (p.effect, p.amount))?;
    let (effect, amount) = pickup;
    if !would_help(world, taker, effect) {
        return None;
    }
    let name = display_name(world, item);
    pay_out_value(world, item);
    let line = apply(world, taker, effect, amount);
    world.entity_mut(item).despawn();
    Some(format!("You pick up the {name}. {line}"))
}

/// A coin-greedy monster (an orc) stepping onto a coin it can actually use —
/// health, magic, a cleared affliction, restored strength. It never touches
/// the two score coins or the two promise coins, which pay off only for the
/// player anyway — [`crate::ai::orc_coin_goal`] only ever points one at a red
/// coin in the first place, but this is what stops an orc that stumbles onto
/// a gold coin mid-chase from "spending" it for nothing.
///
/// Silent: a monster patching itself up is not something the player reads a
/// line about, unlike the player's own pickups. The coin is spent and gone
/// either way. Returns whether anything was actually claimed.
pub(crate) fn monster_claim(world: &mut World, monster: Entity, item: Entity) -> bool {
    let Some((effect, amount)) = world.get::<Pickup>(item).map(|p| (p.effect, p.amount)) else {
        return false;
    };
    let usable = matches!(
        effect,
        PickupEffect::Health | PickupEffect::Power | PickupEffect::Cleanse | PickupEffect::Strength
    );
    if !usable || !would_help(world, monster, effect) {
        return false;
    }
    apply(world, monster, effect, amount);
    world.entity_mut(item).despawn();
    true
}

/// The coin somebody *shot* instead of stepping on: its effect reaches the
/// shooter across the room, and its score with it.
///
/// Called by [`crate::traps::detonate_pickup`] before the burst, so a red coin
/// heals you a beat before your own blast decides what it thinks of you. There
/// is no [`would_help`] gate here: stepping over a coin you cannot use is
/// leaving it for later, but shooting one is a decision, and a decision is
/// allowed to be a waste.
///
/// The coin must still exist — this reads its row off the entity — and the
/// caller destroys it afterwards.
pub(crate) fn claim_from_afar(world: &mut World, shooter: Entity, coin: Entity) {
    let Some((effect, amount)) = world.get::<Pickup>(coin).map(|p| (p.effect, p.amount)) else {
        return;
    };
    let name = display_name(world, coin);
    pay_out_value(world, coin);
    let line = apply(world, shooter, effect, amount);
    if world.get::<Player>(shooter).is_none() {
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} gives itself up to you. {line}"));
}

/// What one coin does. Exhaustive on purpose — a new [`PickupEffect`] with no
/// arm here is a build error, not a coin that silently does nothing.
fn apply(world: &mut World, taker: Entity, effect: PickupEffect, amount: i32) -> String {
    match effect {
        PickupEffect::Coin => "It goes straight into the ledger.".to_string(),
        PickupEffect::Health => heal(world, taker, amount),
        PickupEffect::Power => refill_magic(world, taker, amount),
        PickupEffect::Cleanse => cleanse(world, taker, amount),
        PickupEffect::Strength => restore_strength(world, taker, amount),
        PickupEffect::Platinum => promise(world, taker, Promise::Platinum),
        PickupEffect::Forge => promise(world, taker, Promise::Forge),
    }
}

/// Whether taking this coin would actually do something for `taker`.
///
/// The gate is the pickup category's whole character: a coin you cannot use is
/// a coin you have not spent, and it keeps until you can. Treasure and the two
/// promises always help — there is no such thing as too much score, and a
/// promise you already hold is one the staircase has not settled yet, so
/// [`Promise::already_held`] answers for those.
pub fn would_help(world: &World, taker: Entity, effect: PickupEffect) -> bool {
    match effect {
        PickupEffect::Coin => true,
        PickupEffect::Health => world.get::<Fighter>(taker).is_some_and(|f| f.hp < f.max_hp),
        PickupEffect::Power => world
            .get::<Magic>(taker)
            .is_some_and(|m| m.points < m.max_points),
        PickupEffect::Cleanse => crate::conditions::afflicted(world, taker),
        PickupEffect::Strength => world
            .get::<Fighter>(taker)
            .is_some_and(|f| f.power < f.max_power),
        PickupEffect::Platinum => !Promise::Platinum.already_held(world, taker),
        PickupEffect::Forge => !Promise::Forge.already_held(world, taker),
    }
}

// ---------------------------------------------------------------------------
// The coins that give you something back
// ---------------------------------------------------------------------------

fn heal(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut f) = world.get_mut::<Fighter>(taker) else {
        return String::new();
    };
    let healed = amount.min(f.max_hp - f.hp);
    f.hp += healed;
    format!("Warmth spreads through you. ({healed} HP)")
}

fn refill_magic(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut m) = world.get_mut::<Magic>(taker) else {
        return String::new();
    };
    let gained = (amount as u8).min(m.max_points - m.points);
    m.points += gained;
    format!("Something cold and bright fills your head. ({gained} Ma)")
}

/// The rosé coin: lifts up to `amount` afflictions, worst first, and says how
/// many it got. It stops when there is nothing left to lift, so a player with
/// one condition spends the same coin on one condition.
fn cleanse(world: &mut World, taker: Entity, amount: i32) -> String {
    let lifted = (0..amount)
        .take_while(|_| crate::conditions::cure_one_condition(world, taker))
        .count();
    match lifted {
        1 => "The taste of it clears one thing.".to_string(),
        n => format!("The taste of it clears {n} things."),
    }
}

fn restore_strength(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut f) = world.get_mut::<Fighter>(taker) else {
        return String::new();
    };
    let given = amount.min(f.max_power - f.power);
    f.power += given;
    format!("Your arm remembers what it was. ({given} Pow.)")
}

// ---------------------------------------------------------------------------
// The two that pay later
// ---------------------------------------------------------------------------

/// The two coins whose reward is a promise rather than a thing: named together
/// because everything about them is the same but the payout. One table, two
/// rows, no branching anywhere else.
#[derive(Clone, Copy)]
enum Promise {
    /// A permanent point of attack or defence die.
    Platinum,
    /// A point of plus on the weapon in hand or the armour on your back.
    Forge,
}

impl Promise {
    /// What is logged as the coin is taken.
    fn offer(self) -> &'static str {
        match self {
            Promise::Platinum => "It does not tarnish. Neither, for now, will you. (PLAT)",
            Promise::Forge => "It is still warm. Something is being made. (FORG)",
        }
    }

    /// What is logged when a blow takes it back.
    fn broken(self) -> &'static str {
        match self {
            Promise::Platinum => "The platinum dulls. So much for perfection.",
            Promise::Forge => "The forge goes cold.",
        }
    }

    fn already_held(self, world: &World, taker: Entity) -> bool {
        match self {
            Promise::Platinum => world.get::<Plated>(taker).is_some(),
            Promise::Forge => world.get::<Forged>(taker).is_some(),
        }
    }

    fn attach(self, world: &mut World, taker: Entity) {
        let mut e = world.entity_mut(taker);
        match self {
            Promise::Platinum => e.insert(Plated),
            Promise::Forge => e.insert(Forged),
        };
    }

    fn detach(self, world: &mut World, taker: Entity) {
        let mut e = world.entity_mut(taker);
        match self {
            Promise::Platinum => e.remove::<Plated>(),
            Promise::Forge => e.remove::<Forged>(),
        };
    }

    /// Every promise there is, so the two verbs below never name one twice.
    const ALL: [Promise; 2] = [Promise::Platinum, Promise::Forge];
}

fn promise(world: &mut World, taker: Entity, which: Promise) -> String {
    which.attach(world, taker);
    which.offer().to_string()
}

/// A blow landed on `victim`: any promise it was holding is off. Called from
/// the one place damage is dealt to a creature, so there is no way to be hurt
/// and keep one.
pub fn break_promises(world: &mut World, victim: Entity) {
    for promise in Promise::ALL {
        if !promise.already_held(world, victim) {
            continue;
        }
        promise.detach(world, victim);
        if world.get::<Player>(victim).is_some() {
            world
                .resource_mut::<GameLog>()
                .add(promise.broken().to_string());
        }
    }
}

/// The staircase paying out. Every promise the player still holds is settled
/// here and cleared — this is the one condition a staircase does not simply
/// lift, because reaching the staircase is the whole of what it asked for.
pub fn settle_promises(world: &mut World, player: Entity) {
    for promise in Promise::ALL {
        if !promise.already_held(world, player) {
            continue;
        }
        promise.detach(world, player);
        match promise {
            Promise::Platinum => pay_platinum(world, player),
            Promise::Forge => pay_forge(world, player),
        }
    }
}

/// A permanent point of attack or defence die, the dungeon's coin flip, not
/// yours. The die and not the plus: this is the player growing, and nothing
/// else in the game grows those two numbers.
fn pay_platinum(world: &mut World, player: Entity) {
    let attack = world.resource_mut::<GameRng>().0.r#gen::<bool>();
    let Some(mut f) = world.get_mut::<Fighter>(player) else {
        return;
    };
    let line = match attack {
        true => {
            f.power += 1;
            f.max_power += 1;
            "Untarnished. The platinum goes into your arm. (Pow. +1)"
        }
        false => {
            f.armor += 1;
            "Untarnished. The platinum goes into your hide. (Arm. +1)"
        }
    };
    world.resource_mut::<GameLog>().add(line.to_string());
}

/// A point of plus on the weapon in hand or the armour on the back, whichever
/// the flip picks — falling back to the other when there is only one of them,
/// because a promise kept is a promise kept.
fn pay_forge(world: &mut World, player: Entity) {
    let weapon_first = world.resource_mut::<GameRng>().0.r#gen::<bool>();
    let order = match weapon_first {
        true => [Slot::Hand, Slot::Body],
        false => [Slot::Body, Slot::Hand],
    };
    world
        .resource_mut::<GameLog>()
        .add("The forge collects. Something of yours is finished properly.".to_string());
    for slot in order {
        if crate::items::enchant_equipped(world, player, slot) {
            return;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add("...but you are carrying nothing worth finishing.".to_string());
}
