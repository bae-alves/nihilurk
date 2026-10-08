//! The item *system* — the verbs, not the nouns.
//!
//! Every item that exists is one row in [`crate::catalog`]; that file is where
//! you add content. This module is only what happens when the player acts on
//! one, split by the kind of action:
//!
//! * [`potions`] — quaffing a potion ([`potions::apply_potion_effect`])
//! * [`scrolls`] — reading a scroll ([`scrolls::apply_scroll_effect`])
//! * [`runes`] — invoking a rune ([`runes::apply_rune_effect`]), which goes
//!   inert instead of crumbling and wakes on the stairs
//! * [`wands`] — zapping a wand ([`wands::apply_wand_effect`]), plus the blast
//!   and bolt machinery a thrown wand also borrows
//! * [`throwing`] — hurling anything at anything, and what catches it
//! * [`rings`] — the three rings whose effect is a verb rather than a number
//! * [`pickups`] — coins: what happens the instant you step on one
//! * [`decks`] — drawing a card off a deck, and playing a thrown one as a hand
//!
//! [`item_system`] is the single schedule step: it drains the use-queue, works
//! out what kind of thing each queued item is, manages its physical existence
//! (back in the pack, or to dust), and hands the effect off to the right
//! submodule. [`throw_system`] (re-exported from [`throwing`]) is the sibling
//! step for the throw-queue.
//!
//! Generic, item-agnostic helpers these submodules lean on —
//! [`crate::helpers::item_label`], [`crate::helpers::roll_dice`],
//! [`crate::helpers::free_adjacent_tile`] — live in [`crate::helpers`]. The
//! verbs that put an affliction on or take one off are [`crate::conditions`].

mod decks;
mod pickups;
mod potions;
pub(crate) mod rings;
mod runes;
mod scrolls;
mod spells;
mod theft;
mod throwing;
mod wands;

pub use spells::{can_afford_spell, pay_for_spell, spell_cost, spell_system};
pub use wands::polymorph_player_into;

/// One spell resolved on the spot, cost already settled.
/// [`spell_system`] is the way in for a spell the player triggered; this is
/// the way in for a monster's, which [`pay_for_spell`] first.
pub(crate) use spells::apply_spell_effect;
pub(crate) use wands::leave_smoke_ring;

/// The leprechaun's and the nymph's on-hit tricks, named by
/// [`crate::abilities::ABILITIES`] without that table knowing anything
/// about potions, scrolls or teleportation.
pub(crate) use theft::{leprechaun_theft, nymph_theft};

/// What a blast covers, for anything that must know before it lets one off.
pub(crate) use wands::blast_cells;

pub use decks::{Hand, Rank, score_hand};

/// One blast, resolved. A creature marked to burst when it dies
/// ([`crate::effects::ExplodesOnDeath`]) lets one off through it.
pub(crate) use wands::elemental_blast;

/// What a staircase does to every spent rune in the pack.
pub(crate) use runes::recharge_runes;

pub use throwing::{
    FrozenThrows, ammo_noun, draw_one, drop_refusal, first_matching_ammo, stow, thaw_into_pack,
    throw_reach, throw_refusal, throw_system, time_stopped, use_refusal,
};

/// A launcher-wielding monster's shot, called by [`crate::ai`] in place of a
/// melee attack for as long as it has one drawn.
pub(crate) use throwing::monster_ranged_attack;

pub(crate) use wands::ward_ricochet;

/// The `T` key's whole implementation — the deliberate teleport a ring of
/// teleportation makes possible. Public because the input loop calls it; silent
/// on every path that isn't a jump, because the key is a secret.
pub use rings::{can_teleport_at_will, willed_teleport};

/// Taking something off the floor, in one verb — the pack, the score, and the
/// coins that are spent where they lie. The input handler calls this and knows
/// nothing about any of it.
pub use pickups::{break_promises, pick_up, settle_promises, would_help};

/// A coin claimed by shooting it rather than stepping on it — see
/// [`crate::traps::detonate_pickup`].
pub(crate) use pickups::claim_from_afar;

/// A potion broken by a blast rather than drunk — thrown, or simply caught in
/// somebody else's. See [`crate::traps::chain_react`].
pub(crate) use potions::detonate_potion;

/// The enchantment a forge coin buys, borrowed from the scroll that invented it.
pub(crate) use scrolls::enchant_equipped;

/// Re-exported so the passive-ability table can name them as
/// `crate::items::aggravate_all_monsters` and friends (see
/// [`crate::abilities`]). Everything in that table has the same shape — run on
/// a bearer, report whether it did anything — whichever submodule it lives in.
pub(crate) use rings::{polymorphitis, regenerate, teleportitis};
pub(crate) use scrolls::aggravate_all_monsters;

/// Re-exported so [`crate::combat`] can spend a charmed pair of hands on the
/// blow that lands (a scroll of monster confusion) without knowing what the
/// charm does.
pub(crate) use scrolls::discharge_confusing_touch;

/// The furthest any item can be hurled, re-exported under its historical path so
/// `models::THROW_RANGE` / `crate::items::THROW_RANGE` keep resolving. Defined
/// and documented in [`crate::constants::items`].
pub use crate::constants::items::THROW_RANGE;

/// The furthest a bow or crossbow will carry its own ammunition, and the
/// furthest a potion, scroll, wand or ring flies out of a bare hand — both
/// re-exported alongside [`THROW_RANGE`]. Defined in
/// [`crate::constants::items`].
pub use crate::constants::items::{LAUNCHER_RANGE, LIGHT_THROW_RANGE};

use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::world::World;

use crate::components::*;
use crate::equipment::toggle_equipped;
use crate::helpers::item_label;
use crate::identify::{display_name, with_the};

use self::potions::apply_potion_effect;
use self::runes::apply_rune_effect;
use self::scrolls::apply_scroll_effect;
use self::wands::{apply_wand_effect, is_attack_wand};

/// The schedule step for deep water: anything lying on a
/// [`TileType::Water`](crate::map::TileType::Water) tile sinks, with a splash
/// the log reports if the player saw it go. The Element of Yoord will not
/// sink: it comes up into the player's hands instead (see
/// `pickups::element_surfaces`).
///
/// One step for the whole floor, rather than a check at every place an item
/// can land — thrown, dropped, shaken off a corpse, carried in on a
/// swimmer's back — so a new way to put something down cannot forget the
/// water.
///
/// Assumes nothing upstream. It reads the player's [`Viewshed`] as the last
/// turn left it, because `visibility_system` runs after this step, so a splash
/// is reported against the view the player had when the turn began.
pub fn sink_system(world: &mut World) {
    let seen: Vec<(u16, u16)> = world
        .query_filtered::<&Viewshed, With<Player>>()
        .iter(world)
        .next()
        .map(|v| v.visible_tiles.clone())
        .unwrap_or_default();
    for (at, what) in sink_items(world) {
        if !seen.contains(&(at.x, at.y)) {
            continue;
        }
        world
            .resource_mut::<GameLog>()
            .add(strings::sinks_with_a_splash(&what));
        if let Some(mut fx) = world.get_resource_mut::<crate::particles::Particles>() {
            fx.impact_sparks(at.x, at.y, crossterm::style::Color::Blue, 0.0);
        }
    }
}

/// Sinks every item lying in deep water, and hands the Element of Yoord back
/// to the player if it is one of them. Returns where each sunk item went down
/// and what the player would have called it — for [`sink_system`] to splash
/// about, and for a floor being stocked to ignore, since nobody was there.
pub(crate) fn sink_items(world: &mut World) -> Vec<(Position, String)> {
    let wet: Vec<(Entity, Position)> = {
        let map = world.resource::<crate::map::Map>().clone();
        world
            .query_filtered::<(Entity, &Position), With<Item>>()
            .iter(world)
            .filter(|(_, p)| map.tile(p.x, p.y) == crate::map::TileType::Water)
            .map(|(e, p)| (e, *p))
            .collect()
    };
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next();
    let mut sunk = Vec::new();
    for (item, at) in wet {
        match (world.get::<Amulet>(item).is_some(), player) {
            (true, Some(player)) => pickups::element_surfaces(world, player, item),
            (true, None) => {}
            (false, _) => {
                sunk.push((at, crate::identify::phrase_for(&display_name(world, item))));
                world.despawn(item);
            }
        }
    }
    sunk
}

/// The schedule step that resolves every item the player *used* this turn (as
/// opposed to threw — that is [`throw_system`]).
///
/// For each queued use it captures what the player will see the item called,
/// works out what kind of thing it is, settles where the item physically ends
/// up (back in its exact pack slot, or crumbled to dust once a wand's battery
/// runs dry), logs the "you drink / read / zap it" beat, and only then applies
/// the effect.
///
/// Assumes nothing upstream but a filled [`UseQueue`]: a targeted use was
/// already aimed at the reticle before it was queued, so this only has to
/// resolve what arrives.
pub fn item_system(world: &mut World) {
    let uses = std::mem::take(&mut world.resource_mut::<UseQueue>().uses);
    for item_use in uses {
        crate::equipment::reset_momentum(world, item_use.user);
        resolve_use(world, item_use);
    }
}

/// What a single queued use is: the kinds of thing the item might be, and what
/// should become of it. Filled in from the item's components, then acted on.
#[derive(Default)]
struct UsePlan {
    potion: Option<PotionEffect>,
    wand: Option<WandEffect>,
    /// How many times the wand fires: two for an attack wand in the hands of a
    /// [`DualZap`](crate::effects::DualZap) wearer with the charges to pay.
    casts: u8,
    scroll: Option<ScrollEffect>,
    /// The card just taken off the top of a deck.
    card: Option<Card>,
    /// A rune's effect, and whether it held a charge going in.
    rune: Option<(RuneEffect, bool)>,
    is_equipment: bool,
    destroy: bool,
    keep: bool,
}

/// Reads what `item` is off its components: which effect enum (if any) it
/// carries, whether it is equipment, and — for a wand — whether this zap
/// empties the battery (`destroy`) or leaves charges (`keep`).
fn plan_use(world: &mut World, user: Entity, item: Entity) -> UsePlan {
    let mut plan = UsePlan {
        casts: 1,
        ..UsePlan::default()
    };
    let dual = world.get::<crate::effects::DualZap>(user).is_some();
    let mut e = world.entity_mut(item);
    plan.potion = e.get::<Potion>().map(|p| p.effect);
    plan.wand = e.get::<Wand>().map(|w| w.effect);
    plan.scroll = e.get::<Scroll>().map(|s| s.effect);
    plan.is_equipment = e.get::<crate::equipment::Equipped>().is_some();
    if let Some(mut rune) = e.get_mut::<Rune>() {
        plan.rune = Some((rune.effect, rune.charged));
        rune.charged = false;
        plan.keep = true;
    }
    if let Some(mut battery) = e.get_mut::<Battery>() {
        if battery.charges <= 0 {
            plan.wand = None;
        }
        let doubled = dual && plan.wand.is_some_and(is_attack_wand) && battery.charges >= 2;
        plan.casts = if doubled { 2 } else { 1 };
        battery.charges -= i8::try_from(plan.casts).unwrap_or(1);
        plan.destroy = battery.charges <= 0;
        plan.keep = battery.charges > 0;
    }
    if let Some(mut deck) = e.get_mut::<Deck>() {
        plan.card = deck.cards.pop();
        plan.destroy = deck.cards.is_empty();
        plan.keep = !plan.destroy;
    }
    plan.destroy |= e.get::<Consume>().is_some();
    plan
}

/// Resolves one queued use start to finish: work out what the item is, settle
/// where it physically ends up, log the "you drink / read / zap it" beat, then
/// apply the effect.
fn resolve_use(world: &mut World, item_use: WantsToUse) {
    let seen_name = display_name(world, item_use.item);

    let mut plan = plan_use(world, item_use.user, item_use.item);

    if plan.is_equipment {
        toggle_equipped(world, item_use.user, item_use.item);
        plan.keep = true;
        if world.get::<Consume>(item_use.item).is_some() {
            plan.keep = false;
            plan.destroy = true;
        }
    }

    let inert = !plan.destroy
        && !plan.keep
        && plan.potion.is_none()
        && plan.wand.is_none()
        && plan.scroll.is_none()
        && plan.card.is_none();
    if inert {
        let name = with_the(&item_label(world, item_use.item));
        world
            .resource_mut::<GameLog>()
            .add(strings::cant_use_right_now(&name));
        plan.keep = true;
    }

    if plan.keep && world.entities().contains(item_use.item) {
        return_used_item(world, &item_use);
    }
    if plan.destroy {
        if plan.card.is_none() {
            log_destruction(world, item_use.item, &seen_name);
        }
        world.entity_mut(item_use.item).despawn();
    }
    if !plan.destroy && plan.wand.is_some() {
        world
            .resource_mut::<GameLog>()
            .add(strings::you_zap(&seen_name));
    }

    if let Some(eff) = plan.potion {
        apply_potion_effect(world, item_use.user, eff);
    }
    if let Some(eff) = plan.wand {
        for _ in 0..plan.casts {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
        }
    }
    if let Some(eff) = plan.scroll {
        apply_scroll_effect(world, item_use.user, eff);
    }
    if let Some(card) = plan.card {
        let deck = (!plan.destroy).then_some(item_use.item);
        decks::draw(world, item_use.user, deck, card);
    }
    match plan.rune {
        Some((eff, true)) => {
            world
                .resource_mut::<GameLog>()
                .add(strings::you_invoke(&seen_name));
            apply_rune_effect(world, item_use.user, eff);
        }
        Some((_, false)) => world
            .resource_mut::<GameLog>()
            .add(strings::rune_is_inert().to_string()),
        None => {}
    }
}

/// Puts a used-but-surviving item back in its owner's pack — at its old slot if
/// we still know it, otherwise on the end.
fn return_used_item(world: &mut World, item_use: &WantsToUse) {
    let Some(mut backpack) = world.get_mut::<Backpack>(item_use.user) else {
        return;
    };
    match item_use.slot_idx {
        Some(idx) => {
            let at = idx.min(backpack.items.len());
            backpack.items.insert(at, item_use.item);
        }
        None => backpack.items.push(item_use.item),
    }
}

/// Logs the line for an item consumed by use — a wand crumbling, a potion
/// drunk, a scroll read, or a plain consumable turning to dust.
fn log_destruction(world: &mut World, item: Entity, seen_name: &str) {
    let kinds = (
        world.get::<Wand>(item).is_some(),
        world.get::<Potion>(item).is_some(),
        world.get::<Scroll>(item).is_some(),
        world.get::<Ring>(item).is_some(),
    );
    let mut log = world.resource_mut::<GameLog>();
    match kinds {
        (true, _, _, _) => log.add(strings::wand_crumbles(seen_name)),
        (_, true, _, _) => log.add(strings::you_drink(seen_name)),
        (_, _, true, _) => log.add(strings::you_read(seen_name)),
        (_, _, _, true) => log.add(strings::ring_shivers_apart(seen_name)),
        _ => log.add(strings::item_turns_to_dust()),
    }
}
