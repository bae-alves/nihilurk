//! `Faction::Spirits`: neutral until crossed. A spirit fights nobody on its
//! own — see [`crate::ai::hostile`] and the `MovementType::Confused` wiring
//! on its bestiary row — but two things turn every spirit on the player for
//! good, permanently, for the rest of the run:
//!
//! 1. [`crate::components::Alignment`] reaching a pole (±[`ALIGNMENT_POLE`]): every spirit
//!    that poofs pulls it one step toward its own kind.
//! 2. Any wound on a spirit, whoever dealt it: a blast, a trap, a trick
//!    shot, a monster's claws. Spirits are fickle. See [`on_wounded`].
//!
//! A scroll of atonement makes them neutral again.

use bevy_ecs::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::catalog::{ARMORS, ItemDef, POTIONS, RINGS, SCROLLS, SpellDef, WANDS, WEAPONS};
use crate::components::{
    Alignment, Backpack, BarterColumn, BarterMenu, Curse, Faction, Fighter, GameLog, KnownQuality,
    Magic, Mob, MovementType, OfferMenu, OfferOption, Player, Position, Price, SpellEffect,
    Spellset, SpiritEvent, SpiritKind, SpiritsHostile, TestOfFaithLedger, Tradeable,
};
use crate::constants::spells::SPELLSET_CAP;
use crate::constants::spirits::{
    ALIGNMENT_POLE, ALIGNMENT_STEP, BARTER_STOCK_MAX, BARTER_STOCK_MIN, PINK_DEMON_ODDS_PER_PIECE,
    TEST_OF_FAITH_DUD_BONUS, TEST_OF_FAITH_GEAR_BONUS, TEST_OF_FAITH_HP_DIVISOR,
};
use crate::effects::{ArmorBonus, PowerBonus, revoke_all};
use crate::equipment::{Equipped, Slot, destroy_worn, equipped_items};
use crate::helpers::item_label;
use crate::items::stow;
use crate::map::GameRng;

/// Flips [`SpiritsHostile`] for good, logs the one line it's ever said with,
/// and sends every spirit still standing from its peaceful wander
/// (`MovementType::Confused`) into a proper hunt (`MovementType::Chase`) —
/// otherwise "hostile" would mean nothing but "fights back if bumped into".
/// A no-op if it's already flipped, so every caller can fire this
/// unconditionally.
pub fn challenge_the_balance(world: &mut World) {
    if world.resource::<SpiritsHostile>().0 {
        return;
    }
    world.resource_mut::<SpiritsHostile>().0 = true;
    let spirits: Vec<Entity> = world
        .query::<(Entity, &Faction)>()
        .iter(world)
        .filter(|(_, f)| **f == Faction::Spirits)
        .map(|(e, _)| e)
        .collect();
    for spirit in spirits {
        if let Some(mut mob) = world.get_mut::<Mob>(spirit) {
            mob.movement_type = MovementType::Chase;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::challenge_the_balance());
}

/// The scroll of atonement: the spirits forgive. [`SpiritsHostile`] lifts,
/// every spirit goes back to its peaceful wander, and the player's
/// [`Alignment`] returns to the middle, so the next poof doesn't tip it
/// straight back over the pole. Whoever reads it, the player is the one
/// forgiven.
pub(crate) fn atone(world: &mut World) {
    world.resource_mut::<SpiritsHostile>().0 = false;
    let spirits: Vec<Entity> = world
        .query::<(Entity, &Faction)>()
        .iter(world)
        .filter(|(_, f)| **f == Faction::Spirits)
        .map(|(e, _)| e)
        .collect();
    for spirit in spirits {
        if let Some(mut mob) = world.get_mut::<Mob>(spirit) {
            mob.movement_type = MovementType::Confused;
        }
    }
    let mut players = world.query_filtered::<&mut Alignment, With<Player>>();
    for mut alignment in players.iter_mut(world) {
        alignment.0 = 0;
    }
    world.resource_mut::<GameLog>().add(strings::atonement());
}

/// Every wound runs through here ([`crate::helpers::took_damage`]): blood
/// drawn from a spirit, by anyone or anything, angers them all.
pub(crate) fn on_wounded(world: &mut World, entity: Entity) {
    if world.get::<Faction>(entity) == Some(&Faction::Spirits) {
        challenge_the_balance(world);
    }
}

/// What a peaceful melee against a `Faction::Spirits` mob does instead of a
/// normal attack: runs its [`SpiritEvent`] if it has one. A spirit with no
/// `SpiritEvent` just logs the generic "nothing happens" line. Talking alone
/// never moves [`Alignment`]; a spirit's `poof` does. Called from
/// [`crate::combat::resolve_attack`] before it would otherwise roll damage;
/// never reached once the target isn't peaceful, so this never runs twice for
/// the same swing.
pub fn trigger_event(world: &mut World, player: Entity, spirit: Entity) {
    match world.get::<SpiritEvent>(spirit).copied() {
        Some(SpiritEvent(f)) => f(world, player, spirit),
        None => {
            let name = item_label(world, spirit);
            world
                .resource_mut::<GameLog>()
                .add(strings::spirit_does_nothing(&name));
        }
    }
}

/// Nudges the player's [`Alignment`] the way `spirit`'s [`SpiritKind`]
/// pulls: [`ALIGNMENT_STEP`] toward each pole, nothing for anyone
/// else. A spirit's [`poof`] and the pink demon joining move it, and so
/// does any Helper gained or blown up (see [`crate::companion`]). A spirit
/// that joined the player is no spirit any more: it keeps its kind but not
/// its faction, and pulls nothing when it goes.
fn pull_alignment(world: &mut World, spirit: Entity) {
    if world.get::<Faction>(spirit) != Some(&Faction::Spirits) {
        return;
    }
    let delta = match world.get::<SpiritKind>(spirit) {
        Some(SpiritKind::Cacodaemon) => -ALIGNMENT_STEP,
        Some(SpiritKind::Eudaemon) => ALIGNMENT_STEP,
        None => return,
    };
    shift_player_alignment(world, delta);
}

/// [`shift_alignment`] on the player, whoever that is.
pub(crate) fn shift_player_alignment(world: &mut World, delta: i8) {
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next();
    if let Some(player) = player {
        shift_alignment(world, player, delta);
    }
}

/// Whether `target` is a spirit a peaceful melee should redirect into
/// [`trigger_event`] rather than a normal attack: `Faction::Spirits`, and
/// the spirits haven't turned hostile yet.
pub fn is_peaceful_spirit(world: &World, target: Entity) -> bool {
    world.get::<Faction>(target) == Some(&Faction::Spirits) && !world.resource::<SpiritsHostile>().0
}

/// Nudges the player's [`Alignment`] by `delta` — [`ALIGNMENT_STEP`] either
/// way, by the spirit's kind — clamped to `±`[`ALIGNMENT_POLE`].
/// Reaching either pole challenges the balance the same way a direct hit
/// does. A no-op if `player` has no `Alignment` (never true for the real
/// player, but a scripted test entity might skip it).
pub fn shift_alignment(world: &mut World, player: Entity, delta: i8) {
    let Some(mut alignment) = world.get_mut::<Alignment>(player) else {
        return;
    };
    alignment.0 = (alignment.0 + delta).clamp(-ALIGNMENT_POLE, ALIGNMENT_POLE);
    let at_pole = alignment.0.abs() >= ALIGNMENT_POLE;
    if at_pole {
        challenge_the_balance(world);
    }
}

// ---------------------------------------------------------------------------
// The "choose one of three" menu — the blue demon's spells, the sphynx's
// (traded rather than given, see the barter menu instead), the sylphid's
// weapons, the salamander's armor, undyne's rings.
// ---------------------------------------------------------------------------

/// How many rows an offer menu shows — three, always, per every spirit that
/// uses one.
const OFFER_COUNT: usize = 3;

/// [`sample`], but over a `'static` catalog table too big to copy —
/// [`WEAPONS`]/[`ARMORS`]/[`RINGS`] rows are borrowed, never cloned.
fn sample_refs<T>(
    pool: &'static [T],
    rng: &mut impl rand::RngCore,
    wrap: impl Fn(&'static T) -> OfferOption,
) -> Vec<OfferOption> {
    pool.choose_multiple(rng, OFFER_COUNT).map(wrap).collect()
}

/// Draws `roll` off the shared [`GameRng`], putting it back after. A no-op
/// fallback (nothing offered) in a world with no `GameRng`, the same way
/// `spawn_monster` falls back rather than panicking.
fn roll_with_shared_rng(
    world: &mut World,
    roll: impl FnOnce(&mut rand_chacha::ChaCha12Rng) -> Vec<OfferOption>,
) -> Vec<OfferOption> {
    let Some(GameRng(mut rng)) = world.remove_resource::<GameRng>() else {
        return Vec::new();
    };
    let picked = roll(&mut rng);
    world.insert_resource(GameRng(rng));
    picked
}

/// Three spells the player doesn't already know, off [`SPELLS`] — the same
/// pool [`crate::items::pickups::learn_spell`] draws its one random pick
/// from. Fewer than three (down to none) if that many aren't left to learn.
pub fn roll_spell_offer(world: &mut World, player: Entity) -> Vec<OfferOption> {
    roll_spells(world, player, OFFER_COUNT)
}

/// [`roll_spell_offer`] for the sphynx: [`BARTER_STOCK_MIN`] to
/// [`BARTER_STOCK_MAX`] of them, the count itself rolled.
fn roll_spell_stock(world: &mut World, player: Entity) -> Vec<OfferOption> {
    let count = world
        .resource_mut::<GameRng>()
        .0
        .gen_range(BARTER_STOCK_MIN..=BARTER_STOCK_MAX);
    roll_spells(world, player, count)
}

fn roll_spells(world: &mut World, player: Entity, count: usize) -> Vec<OfferOption> {
    let known: Vec<_> = world
        .get::<Spellset>(player)
        .map(|s| s.slots.clone())
        .unwrap_or_default();
    let learnable: Vec<_> = SpellDef::learnable()
        .filter(|e| !known.contains(e))
        .collect();
    roll_with_shared_rng(world, |rng| {
        let picked = learnable.choose_multiple(rng, count).copied();
        picked.map(OfferOption::Spell).collect()
    })
}

/// Rolls the weapons a spirit's menu offers, a sample of [`WEAPONS`].
pub fn roll_weapon_offer(world: &mut World) -> Vec<OfferOption> {
    roll_with_shared_rng(world, |rng| sample_refs(WEAPONS, rng, OfferOption::Weapon))
}

/// Rolls the armour a spirit's menu offers, a sample of [`ARMORS`].
pub fn roll_armor_offer(world: &mut World) -> Vec<OfferOption> {
    roll_with_shared_rng(world, |rng| sample_refs(ARMORS, rng, OfferOption::Armor))
}

/// Rolls the rings a spirit's menu offers, a sample of [`RINGS`].
pub fn roll_ring_offer(world: &mut World) -> Vec<OfferOption> {
    roll_with_shared_rng(world, |rng| sample_refs(RINGS, rng, OfferOption::Ring))
}

/// One row off each table: the red demon's weapon, armor and ring, or the
/// gnome's scroll, potion and wand.
fn roll_one_of_each(
    world: &mut World,
    roll: impl FnOnce(&mut rand_chacha::ChaCha12Rng) -> [Option<OfferOption>; OFFER_COUNT],
) -> Vec<OfferOption> {
    roll_with_shared_rng(world, |rng| roll(rng).into_iter().flatten().collect())
}

/// Opens the offer menu on `options` — a no-op if there's nothing to offer
/// (an exhausted spell pool), so a spirit event that rolled zero options
/// just says nothing happened rather than raising an empty menu. `source`
/// is the spirit whose event this is, if any — see [`OfferMenu::source`];
/// `priced` is [`OfferMenu::priced`].
pub fn open_offer_menu(
    world: &mut World,
    options: Vec<OfferOption>,
    source: Option<Entity>,
    priced: bool,
) {
    if options.is_empty() {
        if let Some(spirit) = source {
            let name = item_label(world, spirit);
            world
                .resource_mut::<GameLog>()
                .add(strings::spirit_does_nothing(&name));
        }
        return;
    }
    let mut menu = world.resource_mut::<OfferMenu>();
    menu.open = true;
    menu.selected = 0;
    menu.options = options;
    menu.source = source;
    menu.priced = priced;
}

/// The `Z`-menu-style confirm: charges the player if the menu is
/// [`OfferMenu::priced`], applies whichever [`OfferOption`] they picked,
/// closes the menu, and poofs [`OfferMenu::source`] if it had one. A player
/// short of the price is refused and the menu stays up, so a cheaper row is
/// still within reach. Called from the engine's input handler.
pub fn confirm_offer(world: &mut World, player: Entity, choice: OfferOption) {
    let (source, priced) = {
        let menu = world.resource::<OfferMenu>();
        (menu.source, menu.priced)
    };
    let price = choice.price().filter(|_| priced);
    if let Some(price) = price
        && !can_pay(world, player, price)
    {
        world
            .resource_mut::<GameLog>()
            .add(strings::offer_too_dear());
        return;
    }
    world.resource_mut::<OfferMenu>().open = false;
    if let Some(price) = price
        && pay(world, player, price, source)
    {
        return;
    }
    match choice {
        OfferOption::Spell(effect) => learn_offered_spell(world, player, effect),
        OfferOption::Weapon(def) => give_item(world, player, def),
        OfferOption::Armor(def) => give_item(world, player, def),
        OfferOption::Ring(def) => give_item(world, player, def),
        OfferOption::Scroll(def) => give_item(world, player, def),
        OfferOption::Potion(def) => give_item(world, player, def),
        OfferOption::Wand(def) => give_item(world, player, def),
    }
    if let Some(spirit) = source {
        poof(world, spirit);
    }
}

/// Whether `player` has the Max HP or Max Ma `price` asks for. Max HP may be
/// paid down to nothing, which kills them; see [`pay`].
fn can_pay(world: &World, player: Entity, price: Price) -> bool {
    match price {
        Price::MaxHp(n) => world.get::<Fighter>(player).is_some_and(|f| f.max_hp >= n),
        Price::MaxMa(n) => world
            .get::<Magic>(player)
            .is_some_and(|m| m.max_points >= n),
    }
}

/// Takes `price` off `player`'s maximum, current points clamped down to it.
/// Returns whether the payment killed them: their last Max HP gone to
/// `seller`, the deal is off and the run is over.
fn pay(world: &mut World, player: Entity, price: Price, seller: Option<Entity>) -> bool {
    match price {
        Price::MaxHp(n) => {
            let Some(mut f) = world.get_mut::<Fighter>(player) else {
                return false;
            };
            f.max_hp -= n;
            f.hp = f.hp.min(f.max_hp);
            if f.max_hp > 0 {
                return false;
            }
            f.hp = 0;
            let from = seller.and_then(|s| world.get::<Position>(s).copied());
            let killer = seller.map(|s| item_label(world, s)).unwrap_or_default();
            crate::combat::finish_indirect_kill(world, player, from);
            world.resource_mut::<crate::state::Ending>().cause = strings::slain_by(&killer);
            true
        }
        Price::MaxMa(n) => {
            if let Some(mut m) = world.get_mut::<Magic>(player) {
                m.max_points -= n;
                m.points = m.points.min(m.max_points);
            }
            false
        }
    }
}

/// A spirit turning the player away: a grunt, no menu, and the spirit stays
/// put. Every spirit that wants something the player lacks (an item to swap,
/// a spell to swap, the price, gear to destroy) says no this way.
fn refuse(world: &mut World, spirit: Entity) {
    let name = item_label(world, spirit);
    world
        .resource_mut::<GameLog>()
        .add(strings::spirit_refuses(&name));
}

/// Opens a priced offer, or [`refuse`]s a player who can't pay for any row.
fn open_shop(world: &mut World, player: Entity, spirit: Entity, options: Vec<OfferOption>) {
    let affordable = options
        .iter()
        .any(|o| o.price().is_some_and(|p| can_pay(world, player, p)));
    if !affordable {
        refuse(world, spirit);
        return;
    }
    open_offer_menu(world, options, Some(spirit), true);
}

/// Despawns a creature. A spirit goes out with a poof that pulls the
/// player's [`Alignment`], however it leaves: a deal done, a kill, a
/// polymorph, a Helper lost. Every mob despawn calls this rather than
/// `World::despawn`, since a spirit can be any of them.
pub(crate) fn poof(world: &mut World, spirit: Entity) {
    pull_alignment(world, spirit);
    if world.get_entity(spirit).is_some() {
        world.despawn(spirit);
    }
}

fn learn_offered_spell(world: &mut World, player: Entity, effect: SpellEffect) {
    let Some(mut spellset) = world.get_mut::<Spellset>(player) else {
        return;
    };
    if spellset.slots.len() >= SPELLSET_CAP || spellset.slots.contains(&effect) {
        world
            .resource_mut::<GameLog>()
            .add(strings::learn_spell_full());
        return;
    }
    spellset.slots.push(effect);
    let name = crate::catalog::SpellDef::of(effect).display_name();
    world
        .resource_mut::<GameLog>()
        .add(strings::learn_spell_line(name));
}

/// Spawns `def` at the player's feet, already identified, and takes it
/// straight into their pack — or leaves it on the ground if the pack is
/// already full, exactly as any other pickup would.
fn give_item(world: &mut World, player: Entity, def: &'static impl ItemDef) {
    let Some(pos) = world.get::<Position>(player).copied() else {
        return;
    };
    let item = def.spawn(world, pos);
    world.entity_mut(item).insert(KnownQuality);
    stow(world, player, item);
}

// ---------------------------------------------------------------------------
// The barter menu — the yellow demon's items, the sphynx's spells (gdd:
// "same as barter but for spells"). Two columns, the player's own and the
// demon's; picking rows out of each stages them; confirming swaps exactly
// what was staged and poofs the demon, satisfied. Cancelling, or killing the
// demon instead, moves nothing this way — a kill goes through the ordinary
// death-drop path and takes the rest of its pack with it.
// ---------------------------------------------------------------------------

/// Opens an item barter with `demon`: the player's pack against the demon's,
/// nothing pre-selected.
pub fn open_item_barter(world: &mut World, player: Entity, demon: Entity) {
    let player_items = world
        .get::<Backpack>(player)
        .map(|b| b.items.clone())
        .unwrap_or_default();
    let demon_items = world
        .get::<Backpack>(demon)
        .map(|b| b.items.clone())
        .unwrap_or_default();
    open_barter(
        world,
        demon,
        player_items.into_iter().map(Tradeable::Item).collect(),
        demon_items.into_iter().map(Tradeable::Item).collect(),
    );
}

/// Opens a spell barter with `demon` (the sphynx): the player's known spells
/// against `demon_spells`, whatever that row's event fn hands in.
pub fn open_spell_barter(
    world: &mut World,
    player: Entity,
    demon: Entity,
    demon_spells: &[SpellEffect],
) {
    let player_spells = world
        .get::<Spellset>(player)
        .map(|s| s.slots.clone())
        .unwrap_or_default();
    open_barter(
        world,
        demon,
        player_spells.into_iter().map(Tradeable::Spell).collect(),
        demon_spells.iter().copied().map(Tradeable::Spell).collect(),
    );
}

/// With nothing of the player's to put on the table there is no trade to
/// stage, so the demon [`refuse`]s.
fn open_barter(
    world: &mut World,
    demon: Entity,
    player_side: Vec<Tradeable>,
    demon_side: Vec<Tradeable>,
) {
    if player_side.is_empty() {
        refuse(world, demon);
        return;
    }
    let mut menu = world.resource_mut::<BarterMenu>();
    menu.open = true;
    menu.demon = Some(demon);
    menu.column = BarterColumn::Player;
    menu.cursor = 0;
    menu.player_side = player_side;
    menu.demon_side = demon_side;
    menu.player_selected.clear();
    menu.demon_selected.clear();
}

/// Toggles whichever row the cursor is on, in the column it's in: into the
/// staged selection if it wasn't there, out if it was. Called from the
/// engine's input handler.
pub fn toggle_barter_selection(world: &mut World) {
    let mut menu = world.resource_mut::<BarterMenu>();
    let item = match menu.column {
        BarterColumn::Player => menu.player_side.get(menu.cursor).copied(),
        BarterColumn::Demon => menu.demon_visible().get(menu.cursor).copied(),
    };
    let Some(item) = item else {
        return;
    };
    let selected = match menu.column {
        BarterColumn::Player => &mut menu.player_selected,
        BarterColumn::Demon => &mut menu.demon_selected,
    };
    match selected.iter().position(|&t| t == item) {
        Some(pos) => {
            selected.remove(pos);
        }
        None => selected.push(item),
    }
}

/// Leaves the barter without moving anything — the player backed out, or a
/// spirit event opened it and the player just closed it again.
pub fn cancel_barter(world: &mut World) {
    *world.resource_mut::<BarterMenu>() = BarterMenu::default();
}

/// Swaps every staged [`Tradeable`] — the player's selections leave their
/// pack/spellset, the demon's selections join it — then poofs the demon,
/// taking whatever was left unselected in its own pack with it.
pub fn confirm_barter(world: &mut World, player: Entity) {
    let menu = std::mem::take(&mut *world.resource_mut::<BarterMenu>());
    let Some(demon) = menu.demon else {
        return;
    };

    for give in &menu.player_selected {
        take_from_player(world, player, *give);
    }

    let leftover: Vec<Entity> = world
        .get::<Backpack>(demon)
        .map(|b| b.items.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&e| !menu.demon_selected.contains(&Tradeable::Item(e)))
        .collect();

    for take in &menu.demon_selected {
        give_to_player(world, player, *take);
    }

    for item in leftover {
        world.despawn(item);
    }
    poof(world, demon);
    world
        .resource_mut::<GameLog>()
        .add(strings::barter_satisfied());
}

fn take_from_player(world: &mut World, player: Entity, item: Tradeable) {
    match item {
        Tradeable::Item(e) => {
            if let Some(mut bp) = world.get_mut::<Backpack>(player) {
                bp.items.retain(|&x| x != e);
            }
            world.despawn(e);
        }
        Tradeable::Spell(effect) => {
            if let Some(mut s) = world.get_mut::<Spellset>(player) {
                s.slots.retain(|&x| x != effect);
            }
        }
    }
}

fn give_to_player(world: &mut World, player: Entity, item: Tradeable) {
    match item {
        Tradeable::Item(e) => {
            stow(world, player, e);
        }
        Tradeable::Spell(effect) => {
            let Some(mut s) = world.get_mut::<Spellset>(player) else {
                return;
            };
            if s.slots.len() < SPELLSET_CAP && !s.slots.contains(&effect) {
                s.slots.push(effect);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The ten species. Each is a `SpiritEvent` a bestiary row hands to
// `MonsterDef::spirit` — see `monsters.rs`'s `BESTIARY`.
// ---------------------------------------------------------------------------

/// Yellow demon: opens the item barter. A successful trade poofs it
/// (`confirm_barter`); killing it instead drops the rest through the
/// ordinary death-loot path.
pub(crate) fn yellow_demon_event(world: &mut World, player: Entity, spirit: Entity) {
    open_item_barter(world, player, spirit);
}

/// Red demon: a weapon, a suit of armor and a ring, each sold for Max HP.
/// Paying the last of it kills you.
pub(crate) fn red_demon_event(world: &mut World, player: Entity, spirit: Entity) {
    let options = roll_one_of_each(world, |rng| {
        [
            WEAPONS.choose(rng).map(OfferOption::Weapon),
            ARMORS.choose(rng).map(OfferOption::Armor),
            RINGS.choose(rng).map(OfferOption::Ring),
        ]
    });
    open_shop(world, player, spirit, options);
}

/// Gnome: a scroll, a potion and a wand, each sold for Max Ma.
pub(crate) fn gnome_event(world: &mut World, player: Entity, spirit: Entity) {
    let options = roll_one_of_each(world, |rng| {
        [
            SCROLLS.choose(rng).map(OfferOption::Scroll),
            POTIONS.choose(rng).map(OfferOption::Potion),
            WANDS.choose(rng).map(OfferOption::Wand),
        ]
    });
    open_shop(world, player, spirit, options);
}

/// Blue demon: a straight choice of three spells.
pub(crate) fn blue_demon_event(world: &mut World, player: Entity, spirit: Entity) {
    let options = roll_spell_offer(world, player);
    open_offer_menu(world, options, Some(spirit), false);
}

/// Pink demon: destroys every piece of gear the player has on, cursed or
/// not. Each piece gone is [`PINK_DEMON_ODDS_PER_PIECE`] toward them joining
/// as the player's Helper; if they won't join, they vanish. A player with
/// nothing on has nothing to offer, and is [`refuse`]d.
pub(crate) fn pink_demon_event(world: &mut World, player: Entity, spirit: Entity) {
    let worn = equipped_items(world, player);
    if worn.is_empty() {
        refuse(world, spirit);
        return;
    }
    destroy_worn(world, player, &worn);
    let odds = (worn.len() as f64 * PINK_DEMON_ODDS_PER_PIECE).min(1.0);
    if world.resource_mut::<GameRng>().0.gen_bool(odds) {
        pull_alignment(world, spirit);
        crate::companion::recruit(world, spirit);
        world
            .resource_mut::<GameLog>()
            .add(strings::pink_demon_submits());
        return;
    }
    poof(world, spirit);
    world
        .resource_mut::<GameLog>()
        .add(strings::pink_demon_vanishes());
}

/// Angel: "Tests your faith!" — cancels every active effect on the player
/// (the wand of cancellation's own `revoke_all`), divides current HP by [`TEST_OF_FAITH_HP_DIVISOR`] (never
/// below 1), and leaves a [`TestOfFaithLedger`] that pays off on the next
/// staircase (see [`apply_test_of_faith`]). Poofs immediately; there's no
/// menu to wait on.
pub(crate) fn angel_event(world: &mut World, player: Entity, spirit: Entity) {
    revoke_all(world, player);
    if let Some(mut f) = world.get_mut::<Fighter>(player) {
        f.hp = (f.hp / TEST_OF_FAITH_HP_DIVISOR).max(1);
    }
    world.entity_mut(player).insert(TestOfFaithLedger);
    world
        .resource_mut::<GameLog>()
        .add(strings::angel_tests_your_faith());
    poof(world, spirit);
}

/// Sphynx: the same barter menu as the yellow demon's, but for spells — a
/// known spell given up for one of its own, off the same "not already
/// known" pool [`roll_spell_offer`] draws from.
pub(crate) fn sphynx_event(world: &mut World, player: Entity, spirit: Entity) {
    let offered: Vec<SpellEffect> = roll_spell_stock(world, player)
        .into_iter()
        .filter_map(|o| match o {
            OfferOption::Spell(effect) => Some(effect),
            _ => None,
        })
        .collect();
    open_spell_barter(world, player, spirit, &offered);
}

/// Sylphid: a choice of three identified weapons.
pub(crate) fn sylphid_event(world: &mut World, _player: Entity, spirit: Entity) {
    let options = roll_weapon_offer(world);
    open_offer_menu(world, options, Some(spirit), false);
}

/// Salamander: a choice of three identified suits of armor.
pub(crate) fn salamander_event(world: &mut World, _player: Entity, spirit: Entity) {
    let options = roll_armor_offer(world);
    open_offer_menu(world, options, Some(spirit), false);
}

/// Undyne: a choice of three identified rings.
pub(crate) fn undyne_event(world: &mut World, _player: Entity, spirit: Entity) {
    let options = roll_ring_offer(world);
    open_offer_menu(world, options, Some(spirit), false);
}

/// The angel's blessing paying off: every currently **equipped** item only
/// (never the pack, never the floor) — the weapon and armor become a flat
/// [`TEST_OF_FAITH_GEAR_BONUS`] and lose any curse; anything else equipped that was a dud (no
/// enchantment at all) rerolls to a token [`TEST_OF_FAITH_DUD_BONUS`] instead of staying dead
/// weight. A no-op without a pending [`TestOfFaithLedger`], so this is safe
/// to call on every staircase and only ever do something the one time it
/// matters.
pub fn apply_test_of_faith(world: &mut World, player: Entity) {
    if world.get::<TestOfFaithLedger>(player).is_none() {
        return;
    }
    world.entity_mut(player).remove::<TestOfFaithLedger>();
    for item in equipped_items(world, player) {
        let slot = world.get::<Equipped>(item).map(|e| e.slot);
        match slot {
            Some(Slot::Hand) => {
                world
                    .entity_mut(item)
                    .insert((PowerBonus(TEST_OF_FAITH_GEAR_BONUS), KnownQuality));
                world.entity_mut(item).remove::<Curse>();
            }
            Some(Slot::Body) => {
                world
                    .entity_mut(item)
                    .insert((ArmorBonus(TEST_OF_FAITH_GEAR_BONUS), KnownQuality));
                world.entity_mut(item).remove::<Curse>();
            }
            _ => {
                let is_dud = world.get::<PowerBonus>(item).is_none()
                    && world.get::<ArmorBonus>(item).is_none();
                if is_dud {
                    world
                        .entity_mut(item)
                        .insert((ArmorBonus(TEST_OF_FAITH_DUD_BONUS), KnownQuality));
                }
            }
        }
    }
}
