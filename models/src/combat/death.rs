//! What a death sets off: the reaper that sweeps the fallen each turn, the gear and corpse
//! they leave, and the aftershocks (a faerie's reveal, a burst, a biter letting go).
//! Melee kills come through `settle_the_dead`; wands, traps and the rest through
//! `finish_indirect_kill`.

use super::*;

/// Sweeps up anything that has been reduced to 0 HP by a source that doesn't
/// resolve its own lethality — a wand bolt, and any blast casualty
/// `crate::items::wands::elemental_blast` didn't already finish off itself
/// (it does, when it has a blast centre to fling a corpse away from; this is
/// the catch-all for the rest). Melee kills are still finalised inline by
/// [`resolve_attack`], so by the time this runs the only casualties left are
/// indirect ones with no known source to fling a corpse away from.
///
/// Assumes `combat_system` has already run this turn, so any `Fighter.hp <=
/// 0` found here is this turn's business to finish, never a casualty left
/// over from one that already swept.
pub fn reaper_system(world: &mut World) {
    debug_assert!(
        world
            .get_resource::<AttackQueue>()
            .is_none_or(|q| q.attacks.is_empty()),
        "reaper_system ran with attacks still queued: combat_system did not drain them"
    );
    let doomed: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Fighter)>();
        q.iter(world)
            .filter(|(_, f)| f.hp <= 0)
            .map(|(e, _)| e)
            .collect()
    };

    for entity in doomed {
        if world.get_entity(entity).is_none() {
            continue;
        }
        finish_indirect_kill(world, entity, None);
    }
}

/// A tick of recoil for a creature dying where the player can see it: a kill is
/// worth exactly one frame of punctuation, which is why it takes the short kick
/// rather than the heavy one an excellent hit gets. Gated on sight like every
/// other cosmetic, so a monster dying in a room the player has never entered
/// doesn't announce itself through the floor.
///
/// It never steps on a bigger shake: a blast that killed a whole room is
/// already rocking harder than this, and [`Shake::kick`](crate::shake::Shake::kick)
/// keeps whichever is worth more.
pub(super) fn kill_shake(world: &mut World, victim: Entity) {
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    if !player_sees(world, pos.x, pos.y) {
        return;
    }
    kick_shake(world, ShakeKind::Kill);
}

/// Stops the map dead for the rest of the run, called from both player-death
/// paths. A player's death arms no shake of its own, but the blow that killed
/// them may have armed one a frame earlier — a blast's `Heavy`, or the
/// `Wounded` kick from the crossing on the way down — and that one would still
/// be rocking over the slow death burst. Switching the layer off rather than
/// only settling it also means a casualty later in the same blast can't arm a
/// fresh one over a corpse.
pub(super) fn silence_shake(world: &mut World) {
    if let Some(mut shake) = world.get_resource_mut::<Shake>() {
        shake.enabled = false;
        shake.settle();
    }
}

/// Blanks an entity's on-screen glyph to a blank space — used only to hide
/// the player's `@` the instant they die, so the death burst's flung corpse
/// and bone shrapnel read as *them* exploding rather than a corpse detaching
/// from a body still visibly standing there. The player entity is never
/// despawned (the death screen still needs it), so there's nothing to
/// restore it: the run is over.
pub(super) fn blank_player_glyph(world: &mut World, entity: Entity) {
    if let Some(mut r) = world.get_mut::<Renderable>(entity) {
        r.glyph = ' ';
    }
}

/// Finalises one creature that dropped to lethal HP through a source with no
/// attacker entity to report — a wand bolt, or a blast casualty
/// `crate::items::wands::elemental_blast` hands off directly rather than
/// waiting for [`reaper_system`]'s next sweep. Plays the death burst, then
/// either despawns a monster (gear settled first, a plain "dies" line
/// logged) or, for the player, blanks their glyph and flags [`Ending`] —
/// guarded so a player already marked dead this run is neither burst nor
/// re-flagged a second time by a later casualty in the same blast.
///
/// `source` is where the corpse should fly away from — a blast's centre, say
/// — or `None` for a random fling direction.
pub(crate) fn finish_indirect_kill(world: &mut World, entity: Entity, source: Option<Position>) {
    if world.get::<Player>(entity).is_some() {
        if world.resource::<Ending>().player_dead {
            return;
        }
        silence_shake(world);
        death_burst(world, entity, source);
        blank_player_glyph(world, entity);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = strings::mangled_beyond_recognition().to_string();
        return;
    }

    if let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    {
        release_biters_grip(world, entity, player);
    }
    if reveal_faerie(world, entity) {
        return;
    }
    let name = entity_name(world, entity);
    if crate::ice::freezes(world, entity) {
        world
            .resource_mut::<GameLog>()
            .add(strings::mob_freezes_solid(&name));
        pay_for_the_corpse(world, entity);
        leave_gear_behind(world, entity);
        crate::ice::encase(world, entity);
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::mob_dies(&name));
    match world.get::<Helper>(entity).is_some() {
        true => crate::companion::mourn(world, entity),
        false => pay_for_the_corpse(world, entity),
    }
    kill_shake(world, entity);
    death_burst(world, entity, source);
    leave_gear_behind(world, entity);
    burst_on_death(world, entity);
    crate::spirits::poof(world, entity);
}

/// What a corpse is worth, paid into the player's score the moment a creature
/// stops being one ([`crate::score::award_kill`]).
///
/// It never asks whose blade it was. Half the ways a monster dies in nihilurk have
/// no swinger to ask about — a bolt, a blast a room away, a trapdoor it walked
/// into — and a scoreboard that paid for some of those and not others would
/// only be teaching the player to kill things in the approved fashion.
pub(super) fn pay_for_the_corpse(world: &mut World, victim: Entity) {
    let Some(max_hp) = world.get::<Fighter>(victim).map(|f| f.max_hp) else {
        return;
    };
    let mult = match world.get::<ScoreBounty>(victim) {
        Some(_) => BOUNTY_SCORE_MULTIPLIER,
        None => 1,
    };
    award_kill(world, max_hp * mult);
    let name = entity_name(world, victim);
    if let Some(mut kills) = world.get_resource_mut::<Kills>() {
        *kills.by_name.entry(name).or_default() += 1;
    }
    crate::body::feed(world);
}

/// Settles what a dying creature was wearing, item by item. Each piece gets its
/// own [`GEAR_SURVIVES_DEATH`] coin flip: heads it clatters onto the corpse's
/// tile, announced so the player knows there is something to go back for; tails
/// it is destroyed with its owner and never mentioned again.
///
/// This is what stops a thrown dagger an orc caught (see
/// [`crate::items::throw_system`]) from either vanishing silently into the dead
/// entity or coming back every single time.
pub(crate) fn leave_gear_behind(world: &mut World, entity: Entity) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    for item in equipped_items(world, entity) {
        force_unequip(world, item);
        if !world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(GEAR_SURVIVES_DEATH)
        {
            world.entity_mut(item).despawn();
            continue;
        }
        let name = display_name(world, item);
        world.entity_mut(item).insert(pos);
        world
            .resource_mut::<GameLog>()
            .add(strings::gear_clatters_to_floor(&name));
    }
}

/// What is left of a creature the blow killed. A monster pays its score, drops
/// what survives it and leaves the world; the player does none of those things
/// — they stay in it, because the death screen still needs them.
pub(super) fn settle_the_dead(world: &mut World, blow: &Landed) {
    if !blow.outcome.lethal {
        return;
    }
    if blow.target_is_player {
        let attacker_name = entity_name(world, blow.attacker);
        silence_shake(world);
        blank_player_glyph(world, blow.target);
        let mut ending = world.resource_mut::<Ending>();
        ending.player_dead = true;
        ending.cause = strings::slain_by(&attacker_name);
        return;
    }
    release_biters_grip(world, blow.target, blow.attacker);
    let species = crate::monsters::species_of(world, blow.target);
    if !reveal_faerie(world, blow.target) {
        match world.get::<Helper>(blow.target).is_some() {
            true => crate::companion::mourn(world, blow.target),
            false => pay_for_the_corpse(world, blow.target),
        }
        leave_gear_behind(world, blow.target);
        burst_on_death(world, blow.target);
        crate::spirits::poof(world, blow.target);
    }
    crate::monsters::maybe_shapeshift(world, blow.attacker, species);
}

/// A creature marked [`ExplodesOnDeath`](crate::effects::ExplodesOnDeath) (a
/// scroll of justice) goes off as it falls: one fire blast of
/// [`BLAST_RADIUS`](crate::constants::wands::BLAST_RADIUS) where it stood, for
/// one roll of its own power die. It hurts whatever it reaches, the reader
/// included, and a neighbour it kills may burst in turn.
///
/// Called from both ways a monster is finished, [`settle_the_dead`] (a blade)
/// and [`finish_indirect_kill`] (everything else), after its gear is down and
/// before it is gone.
pub(super) fn burst_on_death(world: &mut World, entity: Entity) {
    if world
        .get::<crate::effects::ExplodesOnDeath>(entity)
        .is_none()
    {
        return;
    }
    crate::effects::revoke(
        world,
        entity,
        Grant::of::<crate::effects::ExplodesOnDeath>(),
    );
    let Some(at) = world.get::<Position>(entity).copied() else {
        return;
    };
    let Some(power) = world.get::<Fighter>(entity).map(|f| f.power) else {
        return;
    };
    let damage = crate::helpers::roll_dice(world, 1, power.max(1));
    let fighter = world.entity_mut(entity).take::<Fighter>();
    crate::items::elemental_blast(
        world,
        None,
        at,
        crate::constants::wands::BLAST_RADIUS,
        damage,
        Some(crate::components::Element::Fire),
        crate::particles::BlastPalette::Fire,
    );
    if let Some(fighter) = fighter {
        world.entity_mut(entity).insert(fighter);
    }
}

/// A creature that is a faerie shapeshifter underneath ([`FaerieOnDeath`](crate::FaerieOnDeath), the
/// dog) does not die: it is revealed as one, and gone. It leaves no corpse,
/// no gore and no score, and says so in pink: "It was never a dog, but a
/// faerie shapeshifter!" That line stands in for the kill line. It drops what it wore, as anything does.
/// `true` when it did, and the caller has nothing left to settle.
pub(super) fn reveal_faerie(world: &mut World, entity: Entity) -> bool {
    if world.get::<crate::effects::FaerieOnDeath>(entity).is_none() {
        return false;
    }
    let name = entity_name(world, entity);
    world.resource_mut::<GameLog>().add_colored(
        strings::faerie_reveal(crate::identify::article_for(&name), &name),
        LogCategory::Faerie,
    );
    leave_gear_behind(world, entity);
    crate::spirits::poof(world, entity);
    true
}

/// A grip is the biter's, not the floor's: unlike a bear trap's [`Pinned`],
/// which only turns lift, [`Clamped`] lets go the instant whatever clamped
/// you dies — killing the venus flytrap while it has your leg frees you on
/// the same blow, rather than leaving you thrashing against a corpse until
/// the bite's turns happen to run out.
///
/// Gated on [`ClampedBy`] naming `dead` specifically, not just on `dead`
/// having [`Binds`] — two flytraps can share a room, and killing the one that
/// never bit you must not free you from the one that did.
pub(super) fn release_biters_grip(world: &mut World, dead: Entity, victim: Entity) {
    if world.get::<Binds>(dead).is_some()
        && world
            .get::<ClampedBy>(victim)
            .is_some_and(|by| by.0 == dead)
    {
        revoke(world, victim, Grant::of::<Clamped>());
    }
}
