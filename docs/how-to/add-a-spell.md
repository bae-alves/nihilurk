How to add a spell
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-spell.md` first.
    Result         A new active ability the player can trigger from
                   the `Z` menu, that costs Magic -- an
                   aimed **attack** like a wand, or a self/room-wide
                   **skill** like a potion or a scroll.

One enum variant, one catalog row, one arm of an exhaustive match. The same shape as a potion, a scroll or a wand -- with one difference worth saying up front, and a fork partway through depending on which of the two you're building.


A spell is not an item
----------------------

Every other thing you can pick up in nihilurk is an entity: it has a `Position` on the floor, an `Item` marker, a pack slot, and a fate when you drop or throw it. A spell has none of that. It is never spawned, never lies on a floor, and cannot be dropped or thrown -- there is nothing to drop.

A spell lives as a bare [`SpellEffect`](../../models/src/components.rs) value inside a creature's `Spellset` component:

    pub struct Spellset {
        pub slots: Vec<SpellEffect>,
    }

Today only the player has one, taught in play by a hero coin (an uncommon coin-table row that puts a random, not-yet-known spell straight into the taker's `Spellset`, up to `constants::spells::SPELLSET_CAP` -- four) rather than started with any. Giving a spell to the player for testing without going through the drop table means putting it in that `Vec` by hand -- there is no `NIHILURK_SPAWN` for this, because there is no entity to spawn. See "Hold it in your hands" below.

The other consequence of not being an item: a spell costs [`Magic`](../../models/src/components.rs), not a battery. `SpellDef::cost` is spent by [`spell_system`](../../models/src/items/spells.rs) every time the *player* triggers it. A monster that happens to do the same trick under its own steam -- the dragon's fireball is the one example today -- does not go anywhere near `Spellset`, `SpellQueue` or a Magic cost; it is wired straight into `crate::abilities` (an `ABILITIES` row firing at `Moment::InsteadOfAttacking`) and `crate::items::dragon_breath`, and pays nothing. A `SpellEffect` variant is a shared *mechanic*, not a shared *economy*.


The recipe
----------

Two steps are the same whatever you're building; then the path forks on `SpellKind`.

1. **Append** a variant to `SpellEffect` in `models/src/components.rs` -- at the end, never in the middle (see the warning below):

       pub enum SpellEffect {
           DragonBreath,
           // ... fourteen more ...
           HasteSelf,
           IceBolt,
       }

2. **Add the row** in `SPELLS` (`models/src/catalog.rs`):

       SpellDef { effect: SpellEffect::IceBolt, name: "Ice Bolt", cost: 2, range: 6, kind: SpellKind::Attack },

   `cost` is Magic points, multiplied by `constants::spells::TURBO_MAGIC_COST_MULT` when a staff (`TurboMagic`) is wielded and `kind` is `SpellKind::Attack`, never for a `SpellKind::Skill`. `cost` is also calibrated against `constants::player::START_MAGIC` (4) and sits in one of four Magic-cost tiers, 1 through 4 -- see MANUAL.md, "Magic and spells", for where the existing sixteen land. `range` feeds the aiming reticle exactly like a wand's `Ranged`, and is meaningless -- leave it `0` -- for a spell you're about to make a Skill.

Which of the two you're building decides everything from here.


### Create an attack

An `Attack` opens the aiming reticle, deals damage (or a status a target can shrug off, like Thunderbolt's paralyse), and is the one `TurboMagic` touches at all: a staff multiplies its cost by `constants::spells::TURBO_MAGIC_COST_MULT` and its damage by the larger `TURBO_MAGIC_POWER_MULT`, which is what makes the staff worth the Magic.

3. **Write the mechanic** as one arm of `apply_spell_effect` (`models/src/items/spells.rs`):

       fn apply_spell_effect(world: &mut World, user: Entity, target: Position, effect: SpellEffect, power_mult: i32) {
           match effect {
               SpellEffect::DragonBreath => breathe_fire(world, user, target, power_mult),
               SpellEffect::IceBolt => ice_bolt(world, user, target, power_mult),
           }
       }

   The match has no catch-all, the same as `apply_wand_effect` over `WandEffect` -- step 1's new variant will not build until this step gives it an arm. The compiler names the function and the missing variant, so there is nothing to remember. `power_mult` is `constants::spells::TURBO_MAGIC_POWER_MULT` when a staff multiplied the cost, `1` otherwise -- fold it into your damage roll (multiply the final number, the way every existing `Attack` arm does; see `sting`, `thunderbolt`, `force_lance` for the shape when armour is or isn't subtracted).

   Most of an attack's mechanic is borrowed rather than written:

     * **A blast at a point** -- `breathe_fire`, `lux`, `meteor_strike` all call `elemental_blast` (`items/wands.rs`), the same function a zapped or thrown wand calls. Pick a radius (`BLAST_RADIUS` for a zap-sized disc, `GRENADE_RADIUS` for a thrown-sized one), a damage roll, and an `Element` (or `None` for non-elemental).
     * **A line** -- `force_lance` traces `get_line(user_pos, target)` by hand, stopping at the first wall, and rolls damage per creature it crosses. Copy it when the flavour is "a bolt" or "a beam" rather than "an explosion".
     * **A single target hit like a trap or a wand bolt** -- `sting` (the dart trap's own formula, aimed rather than laid) and `thunderbolt` (a plain armour-ignoring roll, like a wand bolt) both just call `crate::helpers::monster_at(world, target)` and roll once.
     * **Everything in view, no target needed** -- `circle_of_death` and `frost_nova` call `crate::helpers::hostiles_in_view(world, user)` instead of aiming at a point; pair this shape with step 4's "no target" answer below.

   Whichever shape you pick, land the damage through `helpers::apply_hit` with a `Hit::magic` or `Hit::elemental` rather than writing to a `Fighter`'s `hp` yourself -- that one call is what makes Magic Ward, elemental immunity and a petrified victim's damage cap apply to your spell. See "Play fair with Magic Ward" below.

4. **Say whether it needs a target.** Every attack above that hits one aimed tile does; `SpellEffect::needs_target` (`models/src/components.rs`) lists the ones that don't, because they hit everyone in view instead (Circle of Death, Frost Nova) -- add your variant there if that's the shape you picked in step 3, the same way `WandEffect::needs_target` carves the wand of light out from every other wand. Leave it out (needing a target is the default) for anything aimed.


### Create a skill

A `Skill` never opens on damage. It changes the caster (Cure, Heal, Bide, Magic Ward, Haste Self), the floor around them (Setup), or borrows a scroll's own effect outright (Identify, Magic Mapping) -- and `TurboMagic` never touches its cost.

3. **Decide if it needs a target at all.** Almost none do -- a skill works on the caster or on everyone in view, the same as a room-wide attack. Add your variant to `SpellEffect::needs_target`'s `false` list (`models/src/components.rs`) unless you are building the rare aimed skill (nothing in the game does this yet; if you do, treat `target` as step 3 of "Create an attack" would).

4. **Write the mechanic**, the same match arm as an attack, but built one of two ways:

     * **Borrow a scroll wholesale.** Identify and Magic Mapping are one line each:

           SpellEffect::Identify => super::scrolls::apply_scroll_effect(world, user, ScrollEffect::Identify),

       Reach for this whenever an existing scroll already does exactly what you want -- a spell is a delivery method, not a reason to write the mechanic twice.

     * **Write it fresh**, reaching into `crate::conditions` for anything that changes the caster's state (`cure_one_condition`, `hasten`, `paralyse`, ...) the way `cure_self` and `haste_self` do, or lending a marker component through `effects::lend` the way `magic_ward` lends `MagicWard` for `Lifetime::Floor` and `bide` lends `Bided` for `Lifetime::NextAction`. A skill that plants something on the floor (Setup) reaches into `crate::traps` instead -- see `setup` for spawning a revealed trap and springing it immediately if something is already standing there.

   `power_mult` is still a parameter every arm takes, by convention (every arm in the match has the same signature) -- a `Skill` is free to ignore it, since `spell_system` never doubles a `Skill`'s cost.


### Play fair with Magic Ward

Any damage your spell deals to another creature goes through [`helpers::apply_hit`](../../models/src/helpers.rs), the same way every existing damaging spell does:

    if apply_hit(world, victim, Hit::magic(damage), Some(&line)) == 0 {
        return; // or `continue`, in a loop over several victims
    }

`Hit::magic` (no element) and `Hit::elemental` (fire, cold, ...) both set the hit's `magical` flag, and `apply_hit` reads it: a warded victim takes nothing, the log says so, and `wands::ward_ricochet` plays the bounce. `Hit::physical` is the opt-out -- steel, wood and gravity, which a ward is no help against.

No spell checks `MagicWard` by hand, and none should. `apply_hit` owns that question along with elemental immunity and the stone-chip cap, so routing through `wands::elemental_blast` gets all three for free -- the blast calls `apply_hit` per victim itself. The only other ward check in the game is `abilities::fire_on_hit`, and it answers a different question: whether the *trick riding on* a landed blow takes hold. Roll damage straight onto a `Fighter` and your spell becomes the one hole in an otherwise complete shield, which is worse than it sounds: the player *bought* that immunity with three whole Magic points.


Hold it in your hands
----------------------

There is no `NIHILURK_SPAWN` for a spell. In play the only way to learn one is a hero coin (see "A spell is not an item" above); to try Ice Bolt without hunting for a coin, replace the player's starting `Spellset::default()`, in `initialize_world` (`models/src/map/levels.rs`), with:

    Spellset {
        slots: vec![SpellEffect::IceBolt],
    },

Build, run, and press `Z` to see it listed, then its row letter to aim it. `Tab` while aiming snaps the reticle to the next thing in view.

If this was a dry run, `git checkout models/src/map/levels.rs` along with `catalog.rs`, `components.rs` and `items/spells.rs`.


> **Append enum variants; never insert or reorder them.** `SpellEffect` derives `Serialize`, and a save encodes a variant as its position in the enum -- including inside a saved `Spellset` (see the next warning). Adding one at the end is safe; putting one in the middle silently turns a saved spell into a different one, for every save that had it.

> **`Spellset` does survive a save** -- `models/src/saveload.rs` round-trips it like every other piece of the player. So do the two marker components spells have left behind so far, `MagicWard` and `Bided` -- but not through a field of their own: each is a row in the `EFFECTS` registry (`models/src/effects.rs`, `"magic_ward" => MagicWard;`) lent through `effects::lend`, and `EntitySave::effects` saves it by id with the `Lifetime` it is being held for. If your new spell leaves a marker of its own lying around outside `Spellset`, register it there and lend it; a component inserted directly is the one a reload quietly forgets.

> **Nothing routes a spell onto a monster generically.** A `SpellEffect`'s mechanic is shared; the trigger is not. Wiring a species to use one under its own AI, at no Magic cost, is bespoke work in `crate::abilities` -- read how the `FireBreath` row (`Moment::InsteadOfAttacking`) and `dragon_breath` do it for the dragon before assuming a row anywhere makes this automatic.


Verify what you added
----------------------

    cargo build
    cargo test --test spells

There is no `cargo test --test content` coverage for spells the way there is for monsters and items -- `SPELLS` is not part of `spawn_named` or `content_names()`, because nothing about a spell is spawned. `models/tests/spells.rs` pins a spell's *mechanic* instead: it pushes a `WantsToCast` onto `SpellQueue` and runs `spell_system` against a dummy. Copy `sting_bites_like_the_dart_trap_it_borrows_from` if your spell has a number worth holding still.

Then prove it by hand: give it to the player, aim it, and watch the log line `spell_system` prints (`"You cast ___!"`) followed by whatever `apply_spell_effect` logs itself.


See also
--------

  ../tutorial/add-your-first-spell.md   the long version, as a lesson
  add-an-item.md                       the pattern this one borrows
  ../reference/input-and-turn-loop.md  where `spell_system` sits in the schedule
