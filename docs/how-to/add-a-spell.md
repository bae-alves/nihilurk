How to add a spell
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/items/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-spell.md` first.
    Result         A new active ability the player can trigger from
                   the `Z` menu, that costs Magic -- an
                   aimed **attack** like a wand, or a self/room-wide
                   **skill** like a potion or a scroll.

One handler and one catalog row that names it. The same shape as a potion, a scroll or a wand -- with one difference worth saying up front, and a fork partway through depending on which of the two you're building.


A spell is not an item
----------------------

Every other thing you can pick up in nihilurk is an entity: it has a `Position` on the floor, an `Item` marker, a pack slot, and a fate when you drop or throw it. A spell has none of that. It is never spawned, never lies on a floor, and cannot be dropped or thrown -- there is nothing to drop.

A creature's `Spellset` component holds the [`SpellDef`](../../models/src/items/catalog.rs) rows themselves:

    pub struct Spellset {
        pub slots: Vec<&'static SpellDef>,
    }

The player starts with an empty one (a lurk starts with Bide) and learns spells in play: from a hero coin, an uncommon coin-table row that puts a random, not-yet-known spell straight into the taker's `Spellset`, up to `constants::spells::SPELLSET_CAP`, or from a spirit's offer. Monsters that cast hold one too (below). Giving a spell to the player for testing without going through the drop table means putting it in that `Vec` by hand -- there is no `NIHILURK_SPAWN` for this, because there is no entity to spawn. See "Hold it in your hands" below.

The other consequence of not being an item: a spell costs [`Magic`](../../models/src/meta/components/), not a battery. `SpellDef::cost` is spent by [`spell_system`](../../models/src/items/spells.rs) every time the *player* triggers it. A monster that does the same trick under its own steam -- the dragon's fireball, the eel's thunderbolt -- has a `Spellset` and a `Magic` pool of its own, put there by its bestiary row (`MonsterDef::casts`). It pays `SpellDef::cost` out of that pool through the same `spell_cost` the player does (`pay_for_spell`), and it never refills.


The recipe
----------

Two steps are the same whatever you're building; then the path forks on `SpellKind`.

1. **Write the handler** in `models/src/items/spells.rs`. Every handler has the same signature, and takes what it needs out of the `SpellCast`:

       pub(crate) fn ice_bolt(world: &mut World, cast: SpellCast) {
           let SpellCast { user, target, power_mult, .. } = cast;
           // ...
       }

2. **Add the row** to `SPELLS` (`models/src/items/catalog.rs`), naming the handler:

       &SpellDef::new("Ice Bolt", 2, spells::ice_bolt).attack().aimed(6),

   `cost` is Magic points, multiplied by `constants::spells::TURBO_MAGIC_COST_MULT` when a staff (`TurboMagic`) is wielded and the row is `.attack()`, never for a skill. `cost` is also calibrated against `constants::player::START_MAGIC` and sits in one of four Magic-cost tiers, 1 through 4; the rows already in `SPELLS` show where each tier lands. `.aimed(range)` opens the aiming reticle, which reaches `range` tiles exactly like a wand's `Ranged`; a row without it fires the instant its slot is pressed. The handler is required, so a row with none does not build.

   The row's name is the spell's identity: a save stores it and refuses a name no row has. Renaming a row breaks every save that knows the spell.

Which of the two you're building decides everything from here.


### Create an attack

An `Attack` deals damage (or a status a target can shrug off, like Thunderbolt's paralyse), and is the one `TurboMagic` touches at all: a staff multiplies its cost by `constants::spells::TURBO_MAGIC_COST_MULT` and its damage by the larger `TURBO_MAGIC_POWER_MULT`, which is what makes the staff worth the Magic.

3. **Write the mechanic.** `power_mult` is `constants::spells::TURBO_MAGIC_POWER_MULT` when a staff multiplied the cost, `1` otherwise -- fold it into your damage roll (multiply the final number, the way every existing attack does; see `sting`, `thunderbolt`, `force_lance` for the shape when armour is or isn't subtracted).

   Most of an attack's mechanic is borrowed rather than written:

     * **A blast at a point** -- `breathe_fire`, `lux`, `meteor_strike` all call `elemental_blast` (`items/wands.rs`), the same function a zapped or thrown wand calls. Pick a radius (`BLAST_RADIUS` for a zap-sized disc, `GRENADE_RADIUS` for a thrown-sized one), a damage roll, and an `Element` (or `None` for non-elemental).
     * **A line** -- `force_lance` traces `get_line(user_pos, target)` by hand, stopping at the first wall, and rolls damage per creature it crosses. Copy it when the flavour is "a bolt" or "a beam" rather than "an explosion".
     * **A single target hit like a trap or a wand bolt** -- `sting` (the dart trap's own formula, aimed rather than laid) and `thunderbolt` (a plain armour-ignoring roll, like a wand bolt) both just call `crate::helpers::monster_at(world, target)` and roll once.
     * **Everything in view, no target needed** -- `circle_of_death` and `frost_nova` call `crate::helpers::hostiles_in_view(world, user)` instead of aiming at a point; leave `.aimed(..)` off their rows.

   Whichever shape you pick, land the damage through `helpers::apply_hit` with a `Hit::magic` or `Hit::elemental` rather than writing to a `Fighter`'s `hp` yourself -- that one call is what makes Magic Ward, elemental immunity and a petrified victim's damage cap apply to your spell. See "Play fair with Magic Ward" below.

4. **Tell the row its shape.** The reticle and a monster's aim read it off the row, not the handler:

     * `.bursts(radius)` -- the disc the reticle paints on the aimed tile, and the one an ally keeps the player out of.
     * `.spreads(n)` -- how far past that disc a chained impact can land (Meteor Strike).
     * `.line()` -- it flies the line and stops at the first wall (Force Lance).
     * `.wasted_on(f)` -- the reticle passes over a creature `f` says the spell would not change (Fireball skips the fireproof).

   An aimed attack with none of these lands on the aimed tile alone.


### Create a skill

A `Skill` never opens on damage. It changes the caster (Cure, Heal, Bide, Magic Ward, Haste Self), the floor around them (Setup), or borrows a scroll's own effect outright (Identify, Magic Mapping) -- and `TurboMagic` never touches its cost.

3. **Decide if it needs a target at all.** Almost none do -- a skill works on the caster or on everyone in view, the same as a room-wide attack, so its row has no `.aimed(..)`. Polymorph Other is the rare aimed skill: `.aimed(8)` without `.attack()`, so a staff does not double its cost; treat `target` as step 3 of "Create an attack" would.

4. **Write the mechanic**, one of two ways:

     * **Borrow a scroll wholesale.** Identify and Magic Mapping are one line each:

           pub(crate) fn identify(world: &mut World, cast: SpellCast) {
               super::scrolls::identify_everything_hidden(world, cast.user);
           }

       Reach for this whenever an existing scroll already does exactly what you want -- a spell is a delivery method, not a reason to write the mechanic twice.

     * **Write it fresh**, reaching into `crate::conditions` for anything that changes the caster's state (`cure_one_condition`, `hasten`, `paralyse`, ...) the way `cure_self` and `haste_self` do, or lending a marker component through `effects::lend` the way `magic_ward` lends `MagicWard` for `Lifetime::Floor` and `bide` lends `Bided` for `Lifetime::NextAction`. A skill that plants something on the floor (Setup) reaches into `crate::traps` instead -- see `setup` for spawning a revealed trap and springing it immediately if something is already standing there.

   `power_mult` is always `1` for a skill, since `spell_system` never doubles a skill's cost.


### Play fair with Magic Ward

Any damage your spell deals to another creature goes through [`helpers::apply_hit`](../../models/src/combat/helpers.rs), the same way every existing damaging spell does:

    if apply_hit(world, victim, Hit::magic(damage), Some(&line)) == 0 {
        return; // or `continue`, in a loop over several victims
    }

`Hit::magic` (no element) and `Hit::elemental` (fire, cold, ...) both set the hit's `magical` flag, and `apply_hit` reads it: a warded victim takes nothing, the log says so, and `wands::ward_ricochet` plays the bounce. `Hit::physical` is the opt-out -- steel, wood and gravity, which a ward is no help against.

No spell checks `MagicWard` by hand, and none should. `apply_hit` owns that question along with elemental immunity and the stone-chip cap, so routing through `wands::elemental_blast` gets all three for free -- the blast calls `apply_hit` per victim itself. The only other ward check in the game is `abilities::fire_on_hit`, and it answers a different question: whether the *trick riding on* a landed blow takes hold. Roll damage straight onto a `Fighter` and your spell becomes the one hole in an otherwise complete shield, which is worse than it sounds: the player *bought* that immunity with Magic.


Hold it in your hands
----------------------

There is no `NIHILURK_SPAWN` for a spell. In play the only way to learn one is a hero coin (see "A spell is not an item" above); to try Ice Bolt without hunting for a coin, replace the player's starting `Spellset::default()`, in `initialize_world` (`models/src/world/map/levels.rs`), with:

    Spellset {
        slots: vec![spell_row("Ice Bolt").unwrap()],
    },

and add `spell_row` to the `use crate::catalog::{...}` line at the top of `levels.rs`. Build, run, and press `Z` to see it listed, then its row letter to aim it, or `Alt`+`Q`/`W`/`E`/`R` for slots one to four. `Tab` while aiming snaps the reticle to the next thing in view.

Revert the `initialize_world` edit before you run the tests: a player who starts knowing a spell fails the tests that count the starting spellset (the hero coin, amnesia and lurk tests). If this was a dry run, `git checkout models/src/world/map/levels.rs` along with `catalog.rs` and `items/spells.rs`.


> **`Spellset` does survive a save** -- `models/src/meta/saveload.rs` stores each spell by row name and refuses a save that names a spell no row has. So do the two marker components spells have left behind so far, `MagicWard` and `Bided` -- but not through a field of their own: each is a row in the `EFFECTS` registry (`models/src/combat/effects/`, `"magic_ward" => MagicWard;`) lent through `effects::lend`, and `EntitySave::effects` saves it by id with the `Lifetime` it is being held for. If your new spell leaves a marker of its own lying around outside `Spellset`, register it there and lend it; a component inserted directly is the one a reload quietly forgets.

> **A monster casts from its spellset, out of its own Ma.** Its rule set picks one of the spells in its `Spellset` that its `Magic` can pay for, at random each turn, and fires it if it can (`../reference/agents.md`, the `cast` rule). To give a species your spell, give a static list of rows to `.casts(n, &YOUR_LIST)` on its bestiary row (a const row cannot borrow a fresh slice in place; see `BREATHES_FIRE = [SPELLS[FIREBALL]]` in `models/src/creatures/monsters.rs`, `FIREBALL` an `index_of!` const) -- that is all the dragon's Fireball is. Only an `Attack` spell with a range is ever fired this way; a skill never is. A player wearing the row (`-am`) gets the spells in the spell bar and pays from the player's Ma.


Verify what you added
----------------------

    cargo build
    cargo test --test it spells::

There is no `cargo test --test it content::` coverage for spells the way there is for monsters and items -- `SPELLS` is not part of `spawn_named` or `content_names()`, because nothing about a spell is spawned. `models/tests/it/items/spells.rs` pins a spell's *mechanic* instead: it pushes a `WantsToCast` onto `SpellQueue` and runs `spell_system` against a dummy. Copy `sting_bites_like_the_dart_trap_it_borrows_from` if your spell has a number worth holding still.

Then prove it by hand: give it to the player, aim it, and watch the log line `spell_system` prints (`"You cast ___!"`) followed by whatever your handler logs itself.


Appendix: quick check
---------------------

1. Write the handler `fn(&mut World, SpellCast)` in `models/src/items/spells.rs`.
2. Add the `SpellDef::new(name, cost, handler)` row to `SPELLS` in `models/src/items/catalog.rs`.
3. Attack: chain `.attack()`, and land damage through `apply_hit` with `Hit::magic` or `Hit::elemental`.
4. Aimed: chain `.aimed(range)`, plus `.bursts`, `.spreads`, `.line` or `.wasted_on` for its shape.
5. Skill: borrow a scroll's handler if one already does the job.
6. A marker the spell leaves behind: add it to `EFFECTS` and lend it through `effects::lend`.
7. Give it to a species with `.casts(n, &LIST)`, `LIST` a static array of rows; only a ranged attack is ever cast that way.
8. Try it: put it in the starting `Spellset` in `initialize_world`, press `Z`, then revert that edit.
9. Run `cargo test --test it spells::`.


See also
--------

  ../tutorial/add-your-first-spell.md   the long version, as a lesson
  add-an-item.md                       the pattern this one borrows
  ../reference/input-and-turn-loop.md  where `spell_system` sits in the schedule
