//! The numeric effects. A `Modifier` is a component holding a number that stacks across
//! gear; `modifiers!` declares each one and the `Loadout` field it folds into. `loadout`
//! and `equipped_total` do the folding. `MeleeCap` is the one cap, folded with `min`.

use super::*;

// ---------------------------------------------------------------------------
// Numeric modifiers
// ---------------------------------------------------------------------------

/// A number that stacks across every equipped source. Implemented by the
/// modifiers below so [`equipped_total`] can fold any of them with one body.
pub trait Modifier: Component + Copy {
    /// This source's contribution, which may be negative (a cursed blade).
    fn amount(self) -> i32;
}

/// Declares the modifier vocabulary: one row per modifier, giving the
/// component's type name and the [`Loadout`] field it folds into.
///
/// The rows are the only place a modifier is named. From them this generates
/// the component, its [`Modifier`] impl, the matching `Loadout` field and the
/// line of [`Loadout::absorb`] that sums it — so a sixth modifier is a row,
/// not three edits in three places with nothing to catch the one you forgot.
/// That mattered: the fold is a *concrete* struct rather than a generic pass
/// (see [`Loadout`]), and the price of the speed was exactly this kind of
/// hand-kept parallel list.
macro_rules! modifiers {
    ($($(#[$doc:meta])* $name:ident => $field:ident,)*) => {
        $(
            $(#[$doc])*
            #[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
            pub struct $name(pub i32);

            impl Modifier for $name {
                fn amount(self) -> i32 {
                    self.0
                }
            }
        )*

        /// Every number a creature's gear contributes, folded in **one** pass.
        ///
        /// [`equipped_total`] answers for one modifier, which is the right
        /// shape when a caller wants one — a trap measuring armour plus, a
        /// throw measuring its bonus. It is the wrong shape when a caller
        /// wants all of them, because each call walks the wearer's gear again:
        /// combat would make four passes per blow and the HUD five per frame,
        /// over the same handful of items.
        ///
        /// This is the same fold with the loop on the outside. There is no
        /// cache and no second copy of anything — `Equipped` is still the only
        /// record of who is wearing what, and this reads it once instead of
        /// six times.
        ///
        /// # This is a trade, and here is the other side of it
        ///
        /// Concrete fields are what make the single pass possible, and they
        /// cost generality: [`equipped_total::<C>`](equipped_total) takes a new
        /// modifier for free, and this does not. What it does not cost is
        /// *silence* — the struct and the fold are generated from the row
        /// above, so a modifier cannot be added and quietly left out of the
        /// numbers. What is still by hand is the callers that read the fields
        /// they care about; a new field is simply unread until one of them
        /// asks for it, which is a visible nothing rather than a wrong number.
        ///
        /// Worth it at this table size — it took 5.4% off the game frame and
        /// left the ceiling where it was. It stops being worth it if the
        /// vocabulary ever grows past what one pass can usefully carry.
        #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
        pub struct Loadout {
            $(
                #[doc = concat!("Summed [`", stringify!($name), "`] across everything equipped.")]
                pub $field: i32,
            )*
            /// The strictest [`MeleeCap`] anything equipped imposes. Folded
            /// with `min` rather than `+`, because ceilings do not add up —
            /// which is why the cap is written out below instead of riding in
            /// the generated rows.
            pub melee_cap: Option<i32>,
        }

        impl Loadout {
            /// Folds one item's contribution in. The entity's own components
            /// count as well as its gear's — a monster's innate `PowerBonus`
            /// is worth exactly what a ring's is, which is the whole point of
            /// the modifier vocabulary.
            fn absorb(&mut self, item: &EntityRef) {
                $( self.$field += item.get::<$name>().map_or(0, |m| m.0); )*
                if let Some(cap) = item.get::<MeleeCap>().map(|c| c.0) {
                    self.melee_cap = Some(self.melee_cap.map_or(cap, |had| had.min(cap)));
                }
            }
        }
    };
}

modifiers! {
    /// Adds to the bearer's attack **die size**: damage rolls `1d[power]`. This
    /// is what a weapon's class is worth: a dagger's die is small, a two-handed
    /// sword's large.
    PowerDie => power_die,
    /// Flat modifier added once to the bearer's damage roll — an enchantment, or
    /// a ring of strength.
    PowerBonus => power_bonus,
    /// Adds to the bearer's defence **die size**: the armour roll is
    /// `1d[armor]`. This is what a suit of armour is worth.
    ArmorDie => armor_die,
    /// Flat modifier added once to the bearer's armour roll — an enchantment, or
    /// a ring of protection.
    ArmorBonus => armor_bonus,
    /// Flat modifier added once to whatever the bearer *throws* — a ring of
    /// sharpshooting, or the plus on the bow steadying their aim. Folded from every
    /// equipped source the same way the melee bonus is, so it never matters
    /// which piece of gear supplied it.
    ThrowBonus => throw_bonus,
    /// Flat modifier to the bearer's maximum hit points — a ring of health.
    /// Not read per blow: `crate::equipment` reconciles it into
    /// [`Fighter::max_hp`](crate::components::Fighter) when gear changes.
    MaxHpBonus => max_hp_bonus,
    /// A rapier's built-up momentum: [`MOMENTUM_PER_HIT`](crate::constants::abilities::MOMENTUM_PER_HIT) for every consecutive hit it lands,
    /// reset the moment its wielder stops swinging it (see
    /// [`crate::equipment::force_unequip`] and `crate::abilities::build_momentum`).
    /// Kept apart from [`PowerBonus`] so an enchanted rapier's plus and its
    /// momentum never overwrite each other.
    Momentum => momentum,
}

// ---------------------------------------------------------------------------
// One-shots
// ---------------------------------------------------------------------------

/// Something that happens once, the moment this item goes on — the ring of
/// adornment's flourish. Called with `(wearer, item)` by
/// [`crate::equipment::toggle_equipped`] after the item is on and identified,
/// and it owns everything that follows: the log lines, the animation, and
/// tagging the item with [`Consume`](crate::components::Consume) if putting it
/// on is what spends it.
///
/// A [`Grant`] cannot express this — a grant is a property held for as long as
/// the gear is worn, and this is an event — so it rides as its own component,
/// attached by the catalog row the same way grants are.
#[derive(Component, Clone, Copy)]
pub struct OnWear(pub fn(&mut World, Entity, Entity));

/// [`OnWear`]'s twin: something that happens once, the moment this item comes
/// *off* on purpose. Called with `(wearer, item)` by
/// [`crate::equipment::toggle_equipped`] after the item is off and its lent
/// effects are gone.
///
/// Deliberately fired only on the deliberate path, never from
/// [`crate::equipment::force_unequip`] — the same asymmetry [`OnWear`] already
/// has against [`crate::equipment::equip_silently`]. Dying, dropping and being
/// disarmed are not ceremonies, and a line about how you feel is out of place
/// in all three.
///
/// One row uses it: the staff, whose whole effect on a run is invisible on the
/// HUD, so the going and the coming back both have to be said out loud.
#[derive(Component, Clone, Copy)]
pub struct OnDoff(pub fn(&mut World, Entity, Entity));

// ---------------------------------------------------------------------------
// Caps
// ---------------------------------------------------------------------------

/// A ceiling on what the bearer can do in melee, whatever the dice say.
///
/// Not a [`Modifier`]: caps do not add up. Two of them do not make a smaller
/// ceiling than the tighter one alone, so [`Loadout`] folds them with `min`
/// while it is summing everything else.
///
/// A bow carries `MeleeCap(1)`. Drawn, it is the best thing in the dungeon;
/// swung, it is a stick. That is the price of the hand it occupies.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeCap(pub i32);

// ---------------------------------------------------------------------------
// Folding modifiers across equipped gear
// ---------------------------------------------------------------------------

/// [`Loadout`] for `entity`: what it carries itself, plus everything it wears.
pub fn loadout(world: &World, entity: Entity) -> Loadout {
    let mut total = Loadout::default();
    if let Some(own) = world.get_entity(entity) {
        total.absorb(&own);
    }
    for item in crate::equipment::equipped(world, entity) {
        total.absorb(&item);
    }
    total
}

/// The total of modifier `C` across everything `entity` has equipped, plus any
/// it carries itself. The one place *one* number comes from; [`loadout`] is the
/// one place all of them do.
pub fn equipped_total<C: Modifier>(world: &World, entity: Entity) -> i32 {
    let own = world.get::<C>(entity).map(|c| c.amount()).unwrap_or(0);
    let worn: i32 = crate::equipment::equipped(world, entity)
        .filter_map(|item| item.get::<C>().map(|c| c.amount()))
        .sum();
    own + worn
}
