//! The ledger of who holds what, and for how long. `lend` and `revoke` are the only ways
//! to attach or detach an effect at run time; `Lifetime` says when it ends and `Held`
//! is one entry. `attach_effects` rebuilds the ledger on load.

use super::*;

/// How long one held effect lasts, and what ends it.
///
/// Every effect an entity holds carries one of these, so "how long" is asked
/// once, on the row, rather than being implied by which of several parallel
/// structures happened to be storing it. The five cases are the five ways an
/// effect has ever ended in this game; a sixth would be a new variant here and
/// one arm wherever it is ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lifetime {
    /// Never ends on its own. A monster's innate magic, and what the wand of
    /// cancellation is for.
    Permanent,
    /// Lent by a worn item, and lifted when that item comes off. Never
    /// serialised: a loaded save re-lends from the gear itself, so the
    /// [`Entity`] here is only ever meaningful within one session.
    WhileEquipped(Entity),
    /// Until the bearer leaves the floor — a potion of see invisible.
    Floor,
    /// Counts down one per turn and ends at zero. A snare, and every debuff
    /// with a duration after it.
    Turns(u32),
    /// Until the bearer does anything at all — Bide, coiled for one blow.
    NextAction,
}

impl Lifetime {
    /// Whether this lifetime survives being written to a save file. A
    /// gear-lent effect does not: the gear is saved, and lending it again is
    /// how it comes back.
    ///
    /// ```
    /// use bevy_ecs::prelude::Entity;
    /// use models::Lifetime;
    ///
    /// assert!(Lifetime::Permanent.is_saved());
    /// assert!(Lifetime::Turns(3).is_saved());
    /// assert!(!Lifetime::WhileEquipped(Entity::PLACEHOLDER).is_saved());
    /// ```
    pub fn is_saved(self) -> bool {
        !matches!(self, Lifetime::WhileEquipped(_))
    }

    /// Whether what is held this way will stop being held. The other two
    /// lifetimes are what a creature *is*: born with it, or wearing it, and
    /// neither is something that happened to it.
    ///
    /// Half of what [`Held::is_condition`] asks, and the half that has to be
    /// asked of the lifetime rather than the effect: the same `Regenerates`
    /// is a temporary boon out of a potion and a permanent one off a ring.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            Lifetime::Floor | Lifetime::Turns(_) | Lifetime::NextAction
        )
    }
}

/// How many conditions one creature carries at once. A fourth is refused — the
/// body has only so much room to be wrong in, and a player buried under six
/// badges cannot read their own state anyway.
pub const CONDITION_CAP: usize = 3;

/// One effect an entity is holding, and for how long.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Held {
    /// The [`Effect::id`] — a stable string, which is what reaches the disk.
    pub id: &'static str,
    /// What ends it. Only the saved kinds survive a reload; see
    /// [`Lifetime::is_saved`].
    pub lifetime: Lifetime,
    /// The number the grant lent ([`Grant::count`]); `0` for a plain marker.
    pub count: u8,
}

impl Held {
    /// Whether this entry is a *condition*: something the creature is under
    /// right now, and one of the at most [`CONDITION_CAP`] it can be under.
    ///
    /// Both halves have to hold. A ring of regeneration lends a boon that is
    /// not transient, and a potion of magic detection lends a transient mark
    /// that is nothing the marked creature is feeling — neither is a
    /// condition, and neither may crowd a real one out. Which is why the
    /// effect half is [`crate::conditions::is_condition`], read off the lists
    /// that already declare the conditions, rather than a fresh list here:
    /// anything not on one of those lists is exempt by default, which is the
    /// safe direction for a rule that silently takes things away.
    pub fn is_condition(&self) -> bool {
        self.lifetime.is_transient() && crate::conditions::is_condition(self.id)
    }

    /// Whether this entry says what the creature *is* rather than what it can
    /// do — see `IDENTITY_EFFECTS`. The one thing [`revoke_all`] leaves.
    pub fn is_identity(&self) -> bool {
        IDENTITY_EFFECTS.contains(&self.id)
    }
}

impl Grant {
    /// [`Held::is_identity`], asked of the handle: a species' grants are lent
    /// to a polymorphed creature, but what a dog *is* is not.
    pub fn is_identity(&self) -> bool {
        self.effect_id()
            .is_some_and(|id| IDENTITY_EFFECTS.contains(&id))
    }
}

/// Everything an entity is holding: the ledger.
///
/// This records what is attached, from where, and for how long. It does **not**
/// record whether the entity has the property — the marker component does
/// that, the way it always has, and `world.get::<FireImmune>(e)` is still the
/// question the fire code asks.
///
/// One id may appear more than once, and that is the point. A creature born
/// fire-immune and also wearing a ring of fire resistance holds two entries;
/// taking the ring off removes one and the component stays, because the other
/// entry is still there. The three overlapping bitsets this replaced had to
/// intersect each other to work that out at every removal site.
#[derive(Component, Clone, Default, Debug)]
pub struct Effects(pub Vec<Held>);

impl Effects {
    /// Whether any entry still holds `id`.
    pub fn holds(&self, id: &str) -> bool {
        self.0.iter().any(|h| h.id == id)
    }

    /// The entries worth writing to a save file.
    pub fn saved(&self) -> Vec<Held> {
        self.0
            .iter()
            .copied()
            .filter(|h| h.lifetime.is_saved())
            .collect()
    }
}

/// Gives `entity` one effect for as long as `lifetime` says, attaching the
/// component if it is not already there. Returns whether it landed.
///
/// Lending the same effect twice leaves two entries on purpose: two sources,
/// two claims, and losing one must not strip what the other still lends.
///
/// A creature already carrying [`CONDITION_CAP`] conditions refuses another:
/// nothing attaches and this returns `false`. The player is told why; a
/// monster's refusal is silent, the same rule `tick_effects` keeps.
///
/// ```
/// use bevy_ecs::prelude::*;
/// use models::{FireImmune, Grant, Lifetime, lend, revoke, revoke_matching};
///
/// let mut world = World::new();
/// let dragon = world.spawn_empty().id();
/// let fire = Grant::of::<FireImmune>();
///
/// // Two sources lend the same immunity: a birthright, and a potion for the floor.
/// lend(&mut world, dragon, fire, Lifetime::Permanent);
/// lend(&mut world, dragon, fire, Lifetime::Floor);
///
/// // Losing one source leaves the other, so the immunity stays.
/// revoke_matching(&mut world, dragon, |held| held.lifetime == Lifetime::Floor);
/// assert!(fire.probe(&world, dragon));
///
/// revoke(&mut world, dragon, fire);
/// assert!(!fire.probe(&world, dragon));
/// ```
pub fn lend(world: &mut World, entity: Entity, grant: Grant, lifetime: Lifetime) -> bool {
    let Some(id) = grant.effect_id() else {
        return false;
    };
    let held = Held {
        id,
        lifetime,
        count: grant.count(),
    };
    if held.is_condition() && conditions_held(world, entity) >= CONDITION_CAP {
        if world.get::<Player>(entity).is_some() {
            world
                .resource_mut::<GameLog>()
                .add(strings::too_many_conditions());
        }
        return false;
    }
    let mut e = world.entity_mut(entity);
    grant.attach(&mut e);
    let mut ledger = e.take::<Effects>().unwrap_or_default();
    ledger.0.push(held);
    e.insert(ledger);
    true
}

pub(super) fn conditions_held(world: &World, entity: Entity) -> usize {
    world
        .get::<Effects>(entity)
        .map_or(0, |l| l.0.iter().filter(|h| h.is_condition()).count())
}

/// Drops every entry `doomed` accepts, and detaches the component behind any
/// id that no entry holds any more.
///
/// This is the one place an effect is taken away, so the "is anything else
/// still lending it?" question is asked once, here, instead of at each caller.
pub fn revoke_matching(world: &mut World, entity: Entity, doomed: impl Fn(&Held) -> bool) {
    let Some(mut ledger) = world.get_mut::<Effects>(entity).map(|l| l.clone()) else {
        return;
    };
    let lost: Vec<&'static str> = ledger
        .0
        .iter()
        .filter(|h| doomed(h))
        .map(|h| h.id)
        .collect();
    if lost.is_empty() {
        return;
    }
    ledger.0.retain(|h| !doomed(h));

    let mut e = world.entity_mut(entity);
    for id in lost {
        if ledger.holds(id) {
            continue;
        }
        if let Some(effect) = Effect::by_id(id) {
            effect.grant.detach(&mut e);
        }
    }
    e.insert(ledger);
}

/// Takes back every entry lending `grant`, whoever lent it: a charge spent, a
/// hold broken out of. The counterpart to [`lend`], and the reason no caller
/// has to spell an effect's id as a string — a misspelled literal compiles,
/// matches nothing, and revokes nothing, which is the same silence the bare
/// `remove` it replaced used to give.
pub fn revoke(world: &mut World, entity: Entity, grant: Grant) {
    let Some(id) = grant.effect_id() else {
        return;
    };
    revoke_matching(world, entity, |h| h.id == id);
}

/// [`revoke`] over several effects at once, in one pass of the ledger.
pub fn revoke_any(world: &mut World, entity: Entity, grants: &[Grant]) {
    let ids: Vec<&'static str> = grants.iter().filter_map(Grant::effect_id).collect();
    revoke_matching(world, entity, |h| ids.contains(&h.id));
}

/// Lends `entity` one effect until it leaves the floor. Lending twice is
/// harmless — the second dose of the same potion is a second entry, and the
/// staircase takes both.
pub fn grant_for_floor(world: &mut World, entity: Entity, grant: Grant) -> bool {
    lend(world, entity, grant, Lifetime::Floor)
}

/// Gives back everything `entity` was lent for this floor — what a staircase
/// does. Anything the creature was born with, or is wearing, is untouched,
/// because those are their own entries.
pub fn clear_floor_grants(world: &mut World, entity: Entity) {
    revoke_matching(world, entity, |h| h.lifetime == Lifetime::Floor);
}

/// Attaches every effect in `grants` to `entity` — how a monster is born with
/// its innate magic.
pub fn grant_all(world: &mut World, entity: Entity, grants: &'static [Grant]) {
    for g in grants {
        lend(world, entity, *g, Lifetime::Permanent);
    }
}

/// Strips every marker effect `entity` has, from any source, and forgets what
/// it was born with — the wand of cancellation. Walking the ledger means a new
/// effect is cancellable the moment it joins the registry.
pub fn revoke_all(world: &mut World, entity: Entity) {
    revoke_matching(world, entity, |h| !h.is_identity());
    let kept: Vec<Held> = world
        .get::<Effects>(entity)
        .map(|l| l.0.iter().copied().filter(|h| h.is_identity()).collect())
        .unwrap_or_default();
    let mut e = world.entity_mut(entity);
    for effect in EFFECTS {
        if IDENTITY_EFFECTS.contains(&effect.id) {
            continue;
        }
        effect.grant.detach(&mut e);
    }
    e.remove::<Grants>();
    e.remove::<Effects>();
    if !kept.is_empty() {
        e.insert(Effects(kept));
    }
}

/// The effects cancellation cannot take, because they are not magic: they
/// say what the creature *is*. A wand that unmade the lurk would leave
/// something wearing plate armour on four legs.
///
/// The line is drawn between a **body** and a **species**, and it is drawn
/// deliberately:
///
/// * A body — nihil, the lurk — is not cancellable. It is the creature the
///   run is about, and no wand in the dungeon is a wand of being something
///   else.
/// * A species is. A player wearing a dragon ([`crate::body::MonsterBody`])
///   keeps the dragon's shape and its dice, because those are not effects at
///   all, but loses its fire immunity to the same zap that would strip a real
///   dragon's — and an orc-bodied player loses [`ItemUser`], which is to say
///   the wits to work a buckle. That is the bargain of wearing something
///   else's magic: it can be taken off you.
/// * A dog is its grants, and nothing cancels them: they are what the dog
///   is, through any change of shape ([`crate::monsters::DOG_GRANTS`]). The
///   one exception is the shapeshifting itself ([`ShapeshiftOnKill`],
///   [`MirrorOnKill`]): a wand of cancellation stops a dog changing shape.
pub(super) const IDENTITY_EFFECTS: &[&str] = &[
    "lurk",
    "always_tamed",
    "always_helper",
    "priority_helper",
    "faerie_on_death",
];

/// What `entity` holds, for saving. Gear-lent entries are left out; a loaded
/// save lends them again off the gear itself.
pub fn effects_of(world: &World, entity: Entity) -> Vec<Held> {
    world
        .get::<Effects>(entity)
        .map(Effects::saved)
        .unwrap_or_default()
}

/// The number `grant` lends `entity`: the highest of the ledger rows lending it,
/// so a second source can raise it and never lower it. `None` when nothing
/// holds the effect.
pub fn count_of(world: &World, entity: Entity, grant: Grant) -> Option<u8> {
    let id = grant.effect_id()?;
    world
        .get::<Effects>(entity)?
        .0
        .iter()
        .filter(|h| h.id == id)
        .map(|h| h.count)
        .max()
}

/// Puts back what a save recorded.
///
/// An id this build does not have is dropped with a note rather than refused:
/// it means a retired row, or a save from a newer build, and losing one effect
/// beats losing the run. An index-based format could not tell the difference —
/// every index would still have been a valid index.
pub fn attach_effects(entity: &mut EntityWorldMut, held: &[Held]) -> Vec<&'static str> {
    let mut ledger = entity.take::<Effects>().unwrap_or_default();
    let mut unknown = Vec::new();
    for h in held {
        let Some(effect) = Effect::by_id(h.id) else {
            unknown.push(h.id);
            continue;
        };
        effect.grant.attach(entity);
        ledger.0.push(Held {
            id: effect.id,
            lifetime: h.lifetime,
            count: h.count,
        });
    }
    entity.insert(ledger);
    unknown
}
