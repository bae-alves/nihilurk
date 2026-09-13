//! The pack screen: which rows it shows, and what picking one does.
//!
//! There is one list widget and eleven ways into it. `i` opens the whole pack
//! and asks for a verb afterwards, from the Use / Throw / Drop modal. The other
//! ten keys *are* the verb — `a` use, `t` throw, `d` drop, `e` equip, `q`
//! quaff, `r` read, `z` zap, `w` wield, `W` wear, `P` put on — and each narrows
//! the list to the rows that verb can act on, then acts the moment a row is
//! picked. Both halves of that, the narrowing and the verb, hang off
//! [`PackMode`], so the input handler and the renderer ask the same table the
//! same question and can never disagree about what is on screen.
//!
//! **A row keeps its pack letter in every mode.** The potion that is `c` in the
//! pack is `c` in the quaff menu, even when it is the only row there. A letter
//! is a place in the pack, not a place in a list; a menu that renumbered would
//! be teaching the player a second alphabet for every verb. That is why
//! [`pack_rows`] returns *backpack indices* rather than a filtered list of
//! items, and why [`PackIsOpen::selected`] is one of those indices rather than
//! a row number.

use bevy_ecs::prelude::*;

use crate::components::{Backpack, Player, Potion, Scroll, Wand};
use crate::equipment::{Equipped, Slot};

/// What the pack screen can do with the item under the cursor. Nothing else in
/// the game enumerates these.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemAction {
    /// Quaff / read / zap / wear it, depending on what it is.
    Use,
    /// Put it down on the tile you're standing on.
    Drop,
    /// Hurl it at a spot you pick with the aiming reticle.
    Throw,
}

impl ItemAction {
    /// The three rows of the `i` modal, in the order they are offered.
    ///
    /// A fixed order, not a setting. It used to be swappable (`-dropthrow`, for
    /// players who dropped far more often than they threw) and that flag paid
    /// for itself only while the modal was the *only* way to reach any of these
    /// three verbs. It isn't: `a`, `t` and `d` each do one of them in a single
    /// keystroke, so the order of a menu you need never open again is not worth
    /// a command-line flag.
    pub const MENU: [ItemAction; 3] = [ItemAction::Use, ItemAction::Throw, ItemAction::Drop];

    /// The action sitting at menu row `idx`.
    pub fn at(idx: usize) -> ItemAction {
        Self::MENU[idx.min(Self::MENU.len() - 1)]
    }

    /// The label the pack screen paints, padded to the modal's inner width.
    pub fn label(self) -> &'static str {
        match self {
            ItemAction::Use => " Use    ",
            ItemAction::Drop => " Drop   ",
            ItemAction::Throw => " Throw  ",
        }
    }
}

/// Which key opened the pack, and therefore which rows it shows and what
/// picking one does. One row of a table with four columns: [`PackMode::title`],
/// [`PackMode::nothing_line`], [`PackMode::action`] and [`PackMode::admits`].
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PackMode {
    /// `i` — the whole pack, and picking a row opens the action modal.
    #[default]
    Browse,
    /// `a` — the whole pack, and picking a row uses it.
    Use,
    /// `t` — the whole pack, and picking a row opens the aiming reticle.
    Throw,
    /// `d` — the whole pack, and picking a row puts it on the floor.
    Drop,
    /// `e` — anything that can be worn or wielded, whatever the slot.
    Equip,
    /// `q` — only potions.
    Quaff,
    /// `r` — only scrolls.
    Read,
    /// `z` — only wands.
    Zap,
    /// `w` — only what goes in a hand.
    Wield,
    /// `W` — only what goes on the body.
    Wear,
    /// `P` — only what goes on a finger.
    PutOn,
}

impl PackMode {
    /// The heading painted across the top of the box.
    pub fn title(self) -> &'static str {
        match self {
            PackMode::Browse => " INVENTORY ",
            PackMode::Use => " USE WHAT? ",
            PackMode::Throw => " THROW WHAT? ",
            PackMode::Drop => " DROP WHAT? ",
            PackMode::Equip => " EQUIP WHAT? ",
            PackMode::Quaff => " QUAFF WHAT? ",
            PackMode::Read => " READ WHAT? ",
            PackMode::Zap => " ZAP WHAT? ",
            PackMode::Wield => " WIELD WHAT? ",
            PackMode::Wear => " WEAR WHAT? ",
            PackMode::PutOn => " PUT ON WHAT? ",
        }
    }

    /// What the log says instead of opening an empty menu. A menu with no rows
    /// in it is a menu the player has to close again; the refusal says why in
    /// the same keystroke.
    pub fn nothing_line(self) -> &'static str {
        match self {
            PackMode::Browse => "You have no items.",
            PackMode::Use => "You have nothing to use.",
            PackMode::Throw => "You have nothing to throw.",
            PackMode::Drop => "You have nothing to drop.",
            PackMode::Equip => "You have nothing to equip.",
            PackMode::Quaff => "You have nothing to quaff.",
            PackMode::Read => "You have nothing to read.",
            PackMode::Zap => "You have nothing to zap.",
            PackMode::Wield => "You have nothing to wield.",
            PackMode::Wear => "You have nothing to wear.",
            PackMode::PutOn => "You have nothing to put on.",
        }
    }

    /// What picking a row does, or `None` for the one mode that asks first.
    ///
    /// Everything that is not Browse, Throw or Drop is [`ItemAction::Use`]: the
    /// difference between drinking a potion, reading a scroll and buckling on a
    /// breastplate is entirely a matter of what the item is, and `item_system`
    /// already knows. These modes only pick which of them you are offered.
    pub fn action(self) -> Option<ItemAction> {
        match self {
            PackMode::Browse => None,
            PackMode::Throw => Some(ItemAction::Throw),
            PackMode::Drop => Some(ItemAction::Drop),
            PackMode::Use
            | PackMode::Equip
            | PackMode::Quaff
            | PackMode::Read
            | PackMode::Zap
            | PackMode::Wield
            | PackMode::Wear
            | PackMode::PutOn => Some(ItemAction::Use),
        }
    }

    /// Whether `item` belongs on this menu. Gear is anything carrying
    /// [`Equipped`] — a weapon, a suit of armour, a ring — worn or not, since
    /// these menus are also how a piece comes back off.
    ///
    /// The throw menu admits everything, including the two things that will
    /// refuse to leave your hand (the Element of Yoord, cursed worn gear).
    /// Those refusals belong to `throw_refusal`, which says *why* — hiding the
    /// row instead would leave a player wondering where their sword went.
    pub fn admits(self, world: &World, item: Entity) -> bool {
        match self {
            PackMode::Browse | PackMode::Use | PackMode::Throw | PackMode::Drop => true,
            PackMode::Equip => world.get::<Equipped>(item).is_some(),
            PackMode::Quaff => world.get::<Potion>(item).is_some(),
            PackMode::Read => world.get::<Scroll>(item).is_some(),
            PackMode::Zap => world.get::<Wand>(item).is_some(),
            PackMode::Wield => goes_in(world, item, Slot::Hand),
            PackMode::Wear => goes_in(world, item, Slot::Body),
            PackMode::PutOn => goes_in(world, item, Slot::Finger),
        }
    }
}

/// Whether `item` is gear for `slot`.
fn goes_in(world: &World, item: Entity, slot: Slot) -> bool {
    world.get::<Equipped>(item).is_some_and(|e| e.slot == slot)
}

/// Whether the pack modal is open, which mode opened it, and where its two
/// cursors sit — the item row, and (once an item is picked) the action row.
///
/// `selected` and `action_mode` are both *backpack indices*, not row numbers:
/// see this module's header on why a row keeps its pack letter.
#[derive(Resource, Default)]
pub struct PackIsOpen {
    pub open: bool,
    pub mode: PackMode,
    pub selected: usize,
    pub action_mode: Option<usize>,
    pub action_selected: usize,
}

impl PackIsOpen {
    /// Open `mode` with the cursor on backpack row `row`.
    pub fn open_at(&mut self, mode: PackMode, row: usize) {
        self.open = true;
        self.mode = mode;
        self.selected = row;
        self.action_mode = None;
        self.action_selected = 0;
    }

    /// Shut the whole thing, action modal included, and forget the mode.
    pub fn close(&mut self) {
        self.open = false;
        self.mode = PackMode::Browse;
        self.action_mode = None;
    }
}

/// The player's backpack rows `mode` admits, in pack order.
///
/// The one place the filter is applied. The renderer draws exactly these rows,
/// the cursor steps between exactly these rows, and a letter press is accepted
/// only for one of these rows — so a menu can never highlight or act on
/// something it isn't showing.
pub fn pack_rows(world: &mut World, mode: PackMode) -> Vec<usize> {
    let items: Vec<Entity> = world
        .query_filtered::<&Backpack, With<Player>>()
        .iter(world)
        .next()
        .map(|b| b.items.clone())
        .unwrap_or_default();

    items
        .into_iter()
        .enumerate()
        .filter(|&(_, item)| mode.admits(world, item))
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mode, so a mode added without a row here fails to compile.
    const ALL: [PackMode; 11] = [
        PackMode::Browse,
        PackMode::Use,
        PackMode::Throw,
        PackMode::Drop,
        PackMode::Equip,
        PackMode::Quaff,
        PackMode::Read,
        PackMode::Zap,
        PackMode::Wield,
        PackMode::Wear,
        PackMode::PutOn,
    ];

    #[test]
    fn browsing_asks_which_verb_and_every_other_mode_already_knows() {
        assert_eq!(PackMode::Browse.action(), None);
        for mode in ALL.into_iter().filter(|&m| m != PackMode::Browse) {
            assert!(mode.action().is_some(), "{mode:?} has no verb of its own");
        }
        assert_eq!(PackMode::Drop.action(), Some(ItemAction::Drop));
        assert_eq!(PackMode::Throw.action(), Some(ItemAction::Throw));
    }

    #[test]
    fn the_browse_modal_offers_each_verb_exactly_once() {
        // The `i` menu is the one place all three still appear together, and
        // every one of them now has a key of its own as well.
        for action in [ItemAction::Use, ItemAction::Throw, ItemAction::Drop] {
            assert_eq!(
                ItemAction::MENU.iter().filter(|&&a| a == action).count(),
                1,
                "{action:?}"
            );
        }
        assert_eq!(ItemAction::at(0), ItemAction::Use);
        assert_eq!(
            ItemAction::at(99),
            ItemAction::Drop,
            "clamped, not panicking"
        );
    }

    #[test]
    fn every_mode_names_itself_and_says_why_it_is_empty() {
        for mode in ALL {
            assert!(mode.title().starts_with(' '), "{mode:?}");
            assert!(mode.nothing_line().ends_with('.'), "{mode:?}");
        }
    }

    #[test]
    fn an_item_with_none_of_the_marks_is_admitted_only_by_the_unfiltered_modes() {
        // A bare entity stands in for "something the pack holds that is neither
        // potion, scroll, wand nor gear" — a lump of coins, a ration.
        let world = World::new();
        let nothing_in_particular = Entity::from_raw(0);
        let unfiltered = [
            PackMode::Browse,
            PackMode::Use,
            PackMode::Throw,
            PackMode::Drop,
        ];
        for mode in ALL {
            assert_eq!(
                mode.admits(&world, nothing_in_particular),
                unfiltered.contains(&mode),
                "{mode:?}"
            );
        }
    }
}
