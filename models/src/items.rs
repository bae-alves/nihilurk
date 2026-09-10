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

mod potions;
mod scrolls;
mod throwing;
mod wands;

pub use throwing::{draw_one, drop_refusal, stow, throw_refusal, throw_system};

/// Re-exported so the passive-ability table can name it as
/// `crate::items::aggravate_all_monsters` (see [`crate::abilities`]).
pub(crate) use scrolls::aggravate_all_monsters;

/// The furthest any item can be hurled, re-exported under its historical path so
/// `models::THROW_RANGE` / `crate::items::THROW_RANGE` keep resolving. Defined
/// and documented in [`crate::constants::items`].
pub use crate::constants::items::THROW_RANGE;

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
            world
                .resource_mut::<GameLog>()
                .add(format!("You can't use {name} right now."));
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
            world
                .resource_mut::<GameLog>()
                .add(format!("You zap the {seen_name}."));
        }

        // 2. Dispatch to the right submodule. Using a potion, scroll or wand
        // always identifies its true type — every roguelike's use-to-identify
        // convention (rings identify on wear instead, inside `toggle_puton`).
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().potions.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
            if world.resource_mut::<Identified>().wands.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }

        if let Some(eff) = scroll_effect {
            apply_scroll_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().scrolls.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }
    }
}
