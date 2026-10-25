//! Coins, and what stepping on one does.
//!
//! A pickup is the one item the pack never sees: it works where it lies and is
//! gone. That single rule is what the whole module is about —
//!
//! * [`pick_up`] is the *only* way anything leaves the floor for a pack, pickup
//!   or not, so there is one place that knows a coin is answered differently
//!   from a sword. The one exception is the Element of Yoord refusing deep
//!   water ([`element_surfaces`]), which ends in the same payment.
//! * A row's `helps` is asked before a coin is spent, and a coin that would do
//!   nothing is simply not taken. Walking over a red coin at full health leaves
//!   it there for the fight that goes badly, and auto-explore
//!   ([`crate::autoexplore`]) reads the same question before it detours.
//! * Two of them do not pay out now at all. The platinum and forge coins leave
//!   a [`Plated`] / [`Forged`] promise on you, and the staircase settles it
//!   ([`settle_promises`]) — if you got there unhurt.
//!
//! What a coin does is its [`crate::catalog::CoinDef`] row's `apply` and
//! `helps`, both required, so a new coin does not build until it does
//! something. The handlers live here; what they work *with* — how many hit
//! points, how many afflictions — is the row's `amount`, never a number in
//! this file.

use bevy_ecs::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::Color;
use crate::components::*;
use crate::constants::spells::{PROMISE_CAP, SPELLSET_CAP};
use crate::equipment::{Slot, equipped_items, sync_equipment_effects};
use crate::identify::display_name;
use crate::map::{GameRng, Map, SpecialRoom};
use crate::score::award;

// ---------------------------------------------------------------------------
// Taking things off the floor
// ---------------------------------------------------------------------------

/// Everything that happens when `taker` steps onto `item`: the invisible-stash
/// reveal, the score a treasure carries, a pickup's effect, or an ordinary item
/// going into the pack, plus whatever a red room does about it. Returns the
/// line to log, or `None` when the item is left exactly where it was.
pub fn pick_up(world: &mut World, taker: Entity, item: Entity) -> Option<String> {
    let pos = world.get::<Position>(item).copied();
    let line = take_item(world, taker, item)?;
    let Some(pos) = pos else { return Some(line) };
    match ruin_red_room(world, pos, item) {
        Some(extra) => Some(format!("{line} {extra}")),
        None => Some(line),
    }
}

/// The one door onto the floor, so the rules about what can and cannot be
/// carried live together instead of in the input handler. Split out of
/// [`pick_up`] so the red-room consequence above can wrap it without
/// touching this body.
fn take_item(world: &mut World, taker: Entity, item: Entity) -> Option<String> {
    if world.get::<Hidden>(item).is_some() {
        world.entity_mut(item).remove::<Hidden>();
        world.entity_mut(item).remove::<Invisible>();
        world
            .resource_mut::<GameLog>()
            .add(strings::hidden_item_found());
    }

    if world.get::<Pickup>(item).is_some() {
        return spend_pickup(world, taker, item);
    }

    let taken = crate::items::stow(world, taker, item)?;
    pay_out_value(world, item);
    let line = match world.get::<Amulet>(item).is_some() {
        true => strings::take_element_of_yoord().to_string(),
        false => strings::pick_up(&taken),
    };
    Some(line)
}

/// The Element of Yoord will not drown. Whatever put it in deep water, it
/// comes straight up into the player's hands, and everything else they
/// carried — worn or not — burns away in a flash to make room for it. Taken
/// the way [`take_item`] takes it: stowed, and paid for if it never was.
///
/// Nothing the player does can put it there — it can be neither dropped nor
/// thrown — so this is the rule's answer for anything that ever manages to.
pub fn element_surfaces(world: &mut World, player: Entity, element: Entity) {
    let pack = world
        .get::<Backpack>(player)
        .map(|b| b.items.clone())
        .unwrap_or_default();
    for item in pack {
        crate::equipment::force_unequip(world, item);
        world.despawn(item);
    }
    if let Some(mut backpack) = world.get_mut::<Backpack>(player) {
        backpack.items.clear();
    }
    crate::equipment::sync_equipment_effects(world, player);

    crate::items::stow(world, player, element);
    pay_out_value(world, element);
    world
        .resource_mut::<GameLog>()
        .add(strings::element_surfaces());
    if let Some(at) = world.get::<Position>(player).copied()
        && let Some(mut fx) = world.get_resource_mut::<crate::particles::Particles>()
    {
        fx.spark_burst(at.x, at.y, crate::Color::White);
    }
}

/// A pickup inside a red room destroys every other item still in that same
/// room — including one the player dropped or threw in themselves, which
/// counts the same as the room's own treasure. Scoped to the one physical
/// room reachable from `pos` ([`Map::special_room_tiles`]), so a second red
/// room elsewhere on the floor is untouched. `None` (no despawn, no line)
/// when `pos` isn't a red room, or when nothing else was left there to
/// destroy, which is also what makes a second pickup in an already-ruined
/// room a silent no-op.
fn ruin_red_room(world: &mut World, pos: Position, taken: Entity) -> Option<&'static str> {
    let room = {
        let map = world.resource::<Map>();
        if map.special_kind(pos.x, pos.y) != Some(SpecialRoom::RedRoom) {
            return None;
        }
        map.special_room_tiles(pos.x, pos.y)
    };
    let candidates: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), With<Item>>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();
    let others: Vec<Entity> = candidates
        .into_iter()
        .filter(|&(e, p)| e != taken && room.contains(&(p.x, p.y)))
        .map(|(e, _)| e)
        .collect();
    if others.is_empty() {
        return None;
    }
    for e in others {
        world.despawn(e);
    }
    Some(strings::red_room_claimed())
}

/// Pays whatever score an item carries into the run's total as it is taken. A
/// thing with no [`Value`] is worth nothing and says nothing.
///
/// **Once.** The [`Value`] comes off with the payment, so the Element of Yoord
/// pays out once however it comes to hand — picked up, or leaping back out of
/// deep water ([`element_surfaces`]). A coin never needed the rule; it is
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
    let row = world.get::<Pickup>(item)?.row;
    if !(row.helps)(world, taker) {
        return None;
    }
    crate::conduct::player_broke(world, taker, crate::conduct::Conduct::Coins);
    let name = display_name(world, item);
    pay_out_value(world, item);
    let line = (row.apply)(world, taker, row.amount);
    world.entity_mut(item).despawn();
    Some(strings::pick_up_pickup(&name, &line))
}

/// A treasure coin's line: the score itself is its row's `amount`, paid as a
/// [`Value`] when it is taken.
pub fn ledger(_: &mut World, _: Entity, _: i32) -> String {
    strings::coin_ledger().to_string()
}

/// A coin that heals is owed while the taker is hurt.
pub fn hurt(world: &World, taker: Entity) -> bool {
    world.get::<Fighter>(taker).is_some_and(|f| f.hp < f.max_hp)
}

/// A coin that refills magic is owed while the taker's pool is short.
pub fn drained(world: &World, taker: Entity) -> bool {
    world
        .get::<Magic>(taker)
        .is_some_and(|m| m.points < m.max_points)
}

/// A coin that restores strength is owed while the taker's power is down.
pub fn weakened(world: &World, taker: Entity) -> bool {
    world
        .get::<Fighter>(taker)
        .is_some_and(|f| f.power < f.max_power)
}

/// Promises help until the stairs owe you `PROMISE_CAP` of that kind.
pub fn owed_platinum(world: &World, taker: Entity) -> bool {
    Promise::Platinum.held(world, taker) < PROMISE_CAP
}

/// A forge coin is owed while the taker holds fewer promises than the cap.
pub fn owed_forge(world: &World, taker: Entity) -> bool {
    Promise::Forge.held(world, taker) < PROMISE_CAP
}

/// A spell coin is owed while the taker has a spell slot to fill.
pub fn can_learn_spell(world: &World, taker: Entity) -> bool {
    world
        .get::<Spellset>(taker)
        .is_some_and(|m| m.slots.len() < SPELLSET_CAP)
}

// ---------------------------------------------------------------------------
// The coins that give you something back
// ---------------------------------------------------------------------------

/// A coin's heal: `amount` hit points back.
pub fn heal(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut f) = world.get_mut::<Fighter>(taker) else {
        return String::new();
    };
    let healed = amount.min(f.max_hp - f.hp);
    f.hp += healed;
    strings::heal_line(healed)
}

/// A coin's refill: `amount` points of magic back.
pub fn refill_magic(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut m) = world.get_mut::<Magic>(taker) else {
        return String::new();
    };
    let gained = (amount as u8).min(m.max_points - m.points);
    m.points += gained;
    strings::refill_magic_line(gained)
}

/// The rosé coin: lifts up to `amount` afflictions, worst first, and says how
/// many it got. It stops when there is nothing left to lift, so a player with
/// one condition spends the same coin on one condition.
pub fn cleanse(world: &mut World, taker: Entity, amount: i32) -> String {
    let lifted = (0..amount)
        .take_while(|_| crate::conditions::cure_one_condition(world, taker))
        .count();
    match lifted {
        1 => strings::cleanse_one().to_string(),
        n => strings::cleanse_many(n as i32),
    }
}

/// Hero coin: teaches `taker` one random move they don't already know,
/// straight into their [`Spellset`] — always unpredictable, the same way the
/// coin itself is never disguised as anything else. If every spell in the game
/// happens to already be known (all sixteen), the coin has nothing left to
/// give and says so instead of teaching a duplicate.
///
/// The [`SPELLSET_CAP`] is checked *here* rather than left to [`can_learn_spell`].
/// Stepping on a coin is not the only way to reach this: a hero coin that is
/// **shot** pays the shooter nothing, but the cap stays here all the same:
/// no path to this function is allowed to be a fifth spell slot.
pub fn learn_spell(world: &mut World, taker: Entity, _: i32) -> String {
    if world
        .get::<Spellset>(taker)
        .is_none_or(|m| m.slots.len() >= SPELLSET_CAP)
    {
        return strings::learn_spell_full().to_string();
    }
    let known: Vec<&'static crate::catalog::SpellDef> = world
        .get::<Spellset>(taker)
        .map(|m| m.slots.clone())
        .unwrap_or_default();
    let learnable: Vec<&'static crate::catalog::SpellDef> = crate::catalog::SPELLS
        .iter()
        .copied()
        .filter(|s| !known.contains(s))
        .collect();
    if learnable.is_empty() {
        return strings::learn_spell_all_known().to_string();
    }
    let idx = world
        .resource_mut::<GameRng>()
        .0
        .gen_range(0..learnable.len());
    let spell = learnable[idx];
    let Some(mut spellset) = world.get_mut::<Spellset>(taker) else {
        return String::new();
    };
    spellset.slots.push(spell);
    let name = spell.display_name();
    strings::learn_spell_line(name)
}

/// A coin's restore: `amount` points of power back.
pub fn restore_strength(world: &mut World, taker: Entity, amount: i32) -> String {
    let Some(mut f) = world.get_mut::<Fighter>(taker) else {
        return String::new();
    };
    let given = amount.min(f.max_power - f.power);
    f.power += given;
    strings::restore_strength_line(given)
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
    /// A point of plus on the weapon in hand, the armour on your back or a worn
    /// numeric ring.
    Forge,
}

impl Promise {
    /// What is logged as the coin is taken.
    fn offer(self) -> &'static str {
        match self {
            Promise::Platinum => strings::promise_platinum_offer(),
            Promise::Forge => strings::promise_forge_offer(),
        }
    }

    /// What is logged when a blow takes it back.
    fn broken(self) -> &'static str {
        match self {
            Promise::Platinum => strings::promise_platinum_broken(),
            Promise::Forge => strings::promise_forge_broken(),
        }
    }

    /// How many coins of this kind the staircase still owes.
    fn held(self, world: &World, taker: Entity) -> u8 {
        match self {
            Promise::Platinum => world.get::<Plated>(taker).map_or(0, |p| p.0),
            Promise::Forge => world.get::<Forged>(taker).map_or(0, |p| p.0),
        }
    }

    fn attach(self, world: &mut World, taker: Entity) {
        let n = (self.held(world, taker) + 1).min(PROMISE_CAP);
        let mut e = world.entity_mut(taker);
        match self {
            Promise::Platinum => e.insert(Plated(n)),
            Promise::Forge => e.insert(Forged(n)),
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

/// A platinum coin's promise.
pub fn platinum(world: &mut World, taker: Entity, _: i32) -> String {
    promise(world, taker, Promise::Platinum)
}

/// A forge coin's promise.
pub fn forge(world: &mut World, taker: Entity, _: i32) -> String {
    promise(world, taker, Promise::Forge)
}

/// A blow landed on `victim`: any promise it was holding is off. Called from
/// the one place damage is dealt to a creature, so there is no way to be hurt
/// and keep one.
pub fn break_promises(world: &mut World, victim: Entity) {
    for promise in Promise::ALL {
        if promise.held(world, victim) == 0 {
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
        let owed = promise.held(world, player);
        promise.detach(world, player);
        for _ in 0..owed {
            match promise {
                Promise::Platinum => pay_platinum(world, player),
                Promise::Forge => pay_forge(world, player),
            }
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
            strings::pay_platinum_power()
        }
        false => {
            f.armor += 1;
            strings::pay_platinum_armor()
        }
    };
    world.resource_mut::<GameLog>().add(line.to_string());
}

/// A point of plus on the weapon in hand, the armour on the back or a worn
/// ring with a number to it — one of the three, in an order the shuffle picks,
/// falling back to the others when one is missing, because a promise kept is a
/// promise kept.
fn pay_forge(world: &mut World, player: Entity) {
    let mut order = [Some(Slot::Hand), Some(Slot::Body), None];
    order.shuffle(&mut world.resource_mut::<GameRng>().0);
    world
        .resource_mut::<GameLog>()
        .add(strings::pay_forge_collects());
    for slot in order {
        let landed = match slot {
            Some(slot) => crate::items::enchant_equipped(world, player, slot),
            None => enchant_worn_ring(world, player),
        };
        if landed {
            return;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::pay_forge_nothing_worth());
}

/// A point onto a random worn ring that has a number to it (a ring of
/// protection, strength, increase damage, sharpshooting). Returns whether there
/// was one; the forge coin lands here.
fn enchant_worn_ring(world: &mut World, user: Entity) -> bool {
    use crate::effects::{ArmorBonus, MaxHpBonus, PowerBonus, ThrowBonus};
    let numeric: Vec<Entity> = equipped_items(world, user)
        .into_iter()
        .filter(|&e| world.get::<Ring>(e).is_some_and(|r| r.row.is_numeric()))
        .collect();
    let Some(&ring) = numeric.choose(&mut world.resource_mut::<GameRng>().0) else {
        return false;
    };
    let def = world
        .get::<Ring>(ring)
        .map(|r| r.row)
        .expect("filtered to rings");
    let (power, armor, throw, hp) = def.bonuses();
    let mut e = world.entity_mut(ring);
    if power != 0 {
        let base = e.get::<PowerBonus>().map_or(0, |b| b.0);
        e.insert(PowerBonus(base + 1));
    }
    if armor != 0 {
        let base = e.get::<ArmorBonus>().map_or(0, |b| b.0);
        e.insert(ArmorBonus(base + 1));
    }
    if throw != 0 {
        let base = e.get::<ThrowBonus>().map_or(0, |b| b.0);
        e.insert(ThrowBonus(base + 1));
    }
    if hp != 0 {
        let base = e.get::<MaxHpBonus>().map_or(0, |b| b.0);
        e.insert(MaxHpBonus(base + 1));
    }
    sync_equipment_effects(world, user);
    world.entity_mut(ring).insert(KnownQuality);
    let name = display_name(world, ring);
    crate::helpers::spark_burst_at(world, user, Color::DarkYellow);
    world
        .resource_mut::<GameLog>()
        .add(strings::enchant_sparks(&name));
    true
}
