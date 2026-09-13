//! The item *system* — the verbs, not the nouns.
//!
//! Every item that exists is one row in [`crate::catalog`]; that file is where
//! you add content. This module is only what happens when the player acts on
//! one, split by the kind of action:
//!
//! * [`potions`] — quaffing a potion ([`potions::apply_potion_effect`])
//! * [`scrolls`] — reading a scroll ([`scrolls::apply_scroll_effect`])
//! * [`wands`] — zapping a wand ([`wands::apply_wand_effect`]), plus the blast
//!   and bolt machinery a thrown wand also borrows
//! * [`throwing`] — hurling anything at anything, and what catches it
//! * [`rings`] — the three rings whose effect is a verb rather than a number
//! * [`pickups`] — coins: what happens the instant you step on one
//!
//! [`item_system`] is the single schedule step: it drains the use-queue, works
//! out what kind of thing each queued item is, manages its physical existence
//! (back in the pack, or to dust), and hands the effect off to the right
//! submodule. [`throw_system`] (re-exported from [`throwing`]) is the sibling
//! step for the throw-queue.
//!
//! Generic, item-agnostic helpers these submodules lean on —
//! [`crate::helpers::item_label`], [`crate::helpers::roll_dice`],
//! [`crate::helpers::free_adjacent_tile`],
//! [`crate::helpers::clear_player_conditions`] — live in [`crate::helpers`].

mod pickups;
mod potions;
pub(crate) mod rings;
mod scrolls;
mod throwing;
mod wands;

pub use throwing::{
    ammo_noun, draw_one, drop_refusal, first_matching_ammo, stow, throw_refusal, throw_system,
};

/// The `T` key's whole implementation — the deliberate teleport a ring of
/// teleportation makes possible. Public because the input loop calls it; silent
/// on every path that isn't a jump, because the key is a secret.
pub use rings::willed_teleport;

/// Taking something off the floor, in one verb — the pack, the score, and the
/// coins that are spent where they lie. The input handler calls this and knows
/// nothing about any of it.
pub use pickups::{break_promises, pick_up, settle_promises, would_help};

/// A coin claimed by shooting it rather than stepping on it — see
/// [`crate::traps::detonate_pickup`].
pub(crate) use pickups::claim_from_afar;

/// The enchantment a forge coin buys, borrowed from the scroll that invented it.
pub(crate) use scrolls::enchant_equipped;

/// Re-exported so the passive-ability table can name them as
/// `crate::items::aggravate_all_monsters` and friends (see
/// [`crate::abilities`]). Everything in that table has the same shape — run on
/// a bearer, report whether it did anything — whichever submodule it lives in.
pub(crate) use rings::{regenerate, teleportitis};
pub(crate) use scrolls::aggravate_all_monsters;

/// Re-exported so [`crate::combat`] can spend a charmed pair of hands on the
/// blow that lands (a scroll of monster confusion) without knowing what the
/// charm does.
pub(crate) use scrolls::discharge_confusing_touch;

/// The furthest any item can be hurled, re-exported under its historical path so
/// `models::THROW_RANGE` / `crate::items::THROW_RANGE` keep resolving. Defined
/// and documented in [`crate::constants::items`].
pub use crate::constants::items::THROW_RANGE;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::components::*;
use crate::equipment::toggle_equipped;
use crate::helpers::item_label;
use crate::identify::Identified;

use self::potions::apply_potion_effect;
use self::scrolls::apply_scroll_effect;
use self::wands::apply_wand_effect;

/// The schedule step that resolves every item the player *used* this turn (as
/// opposed to threw — that is [`throw_system`]).
///
/// For each queued use it captures what the player will see the item called,
/// works out what kind of thing it is, settles where the item physically ends
/// up (back in its exact pack slot, or crumbled to dust once a wand's battery
/// runs dry), logs the "you drink / read / zap it" beat, and only then applies
/// the effect. Using a potion, scroll or wand always identifies its true type —
/// the classic use-to-identify convention; rings identify on wear instead,
/// inside [`crate::equipment`].
pub fn item_system(world: &mut World) {
    let uses = std::mem::take(&mut world.resource_mut::<UseQueue>().uses);
    for item_use in uses {
        resolve_use(world, item_use);
    }
}

/// What a single queued use is: the kinds of thing the item might be, and what
/// should become of it. Filled in from the item's components, then acted on.
#[derive(Default)]
struct UsePlan {
    potion: Option<PotionEffect>,
    wand: Option<WandEffect>,
    scroll: Option<ScrollEffect>,
    is_equipment: bool,
    destroy: bool,
    keep: bool,
}

/// Reads what `item` is off its components: which effect enum (if any) it
/// carries, whether it is equipment, and — for a wand — whether this zap
/// empties the battery (`destroy`) or leaves charges (`keep`).
fn plan_use(world: &mut World, item: Entity) -> UsePlan {
    let mut plan = UsePlan::default();
    let mut e = world.entity_mut(item);
    plan.potion = e.get::<Potion>().map(|p| p.effect);
    plan.wand = e.get::<Wand>().map(|w| w.effect);
    plan.scroll = e.get::<Scroll>().map(|s| s.effect);
    plan.is_equipment = e.get::<crate::equipment::Equipped>().is_some();
    if let Some(mut battery) = e.get_mut::<Battery>() {
        battery.charges -= 1;
        plan.destroy = battery.charges <= 0;
        plan.keep = battery.charges > 0;
    }
    plan.destroy |= e.get::<Consume>().is_some();
    plan
}

/// Resolves one queued use start to finish: work out what the item is, settle
/// where it physically ends up, log the "you drink / read / zap it" beat, then
/// apply the effect (which also identifies the type on first use).
fn resolve_use(world: &mut World, item_use: WantsToUse) {
    // What the player sees it called now, and its true name — captured before a
    // despawn below could make the entity unqueryable.
    let seen_name = crate::identify::display_name(world, item_use.item);
    let true_name = item_label(world, item_use.item);

    let mut plan = plan_use(world, item_use.item);

    // Equipment toggles its equipped state and always goes back in the pack —
    // unless wearing it is what spent it. A ring of adornment fires the moment
    // it goes on and tags itself [`Consume`] on the way out, so the plan is
    // asked again afterwards: it is the one item whose fate is decided *by*
    // being equipped rather than before.
    if plan.is_equipment {
        toggle_equipped(world, item_use.user, item_use.item);
        plan.keep = true;
        if world.get::<Consume>(item_use.item).is_some() {
            plan.keep = false;
            plan.destroy = true;
        }
    }

    // Anything the game can't "use" is handed straight back, not lost.
    let inert = !plan.destroy
        && !plan.keep
        && plan.potion.is_none()
        && plan.wand.is_none()
        && plan.scroll.is_none();
    if inert {
        let name = crate::identify::with_the(&item_label(world, item_use.item));
        world
            .resource_mut::<GameLog>()
            .add(format!("You can't use {name} right now."));
        plan.keep = true;
    }

    if plan.keep {
        return_used_item(world, &item_use);
    }
    if plan.destroy {
        log_destruction(world, item_use.item, &seen_name);
        world.entity_mut(item_use.item).despawn();
    }
    if !plan.destroy && plan.wand.is_some() {
        // A wand survives its zap, so its "you use it" beat lives here.
        world
            .resource_mut::<GameLog>()
            .add(format!("You zap the {seen_name}."));
    }

    // Dispatch to the right submodule; each apply identifies the type on the
    // first use (rings identify on wear instead, inside `crate::equipment`).
    if let Some(eff) = plan.potion {
        apply_potion_effect(world, item_use.user, eff);
        let newly = world.resource_mut::<Identified>().potions.insert(eff);
        announce_first_id(world, newly, &true_name);
    }
    if let Some(eff) = plan.wand {
        apply_wand_effect(world, item_use.user, item_use.target, eff);
        let newly = world.resource_mut::<Identified>().wands.insert(eff);
        announce_first_id(world, newly, &true_name);
    }
    if let Some(eff) = plan.scroll {
        apply_scroll_effect(world, item_use.user, eff);
        let newly = world.resource_mut::<Identified>().scrolls.insert(eff);
        announce_first_id(world, newly, &true_name);
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
        (true, _, _, _) => log.add(format!("The {seen_name} crumbles to dust!")),
        (_, true, _, _) => log.add(format!("You drink the {seen_name}.")),
        (_, _, true, _) => log.add(format!("You read the {seen_name}.")),
        // Only one ring is ever spent this way, and it does not merely crumble:
        // it goes out the way it came in.
        (_, _, _, true) => log.add(format!(
            "The {seen_name} shivers apart into a thousand glittering motes."
        )),
        _ => log.add("The item turns to dust!".to_string()),
    }
}

/// Logs the reveal line the first time an item type is identified by use.
fn announce_first_id(world: &mut World, newly_identified: bool, true_name: &str) {
    if !newly_identified {
        return;
    }
    world.resource_mut::<GameLog>().add(format!(
        "That was {} {true_name}!",
        crate::identify::article_for(true_name)
    ));
}
