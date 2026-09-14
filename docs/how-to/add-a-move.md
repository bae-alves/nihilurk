How to add a move
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-move.md` first.
    Result         A new active ability the player can trigger with
                   `Z` or `Alt`+a letter, that costs Magic -- an
                   aimed **attack** like a wand, or a self/room-wide
                   **skill** like a potion or a scroll.

One enum variant, one catalog row, one arm of an exhaustive match. The same shape as a potion, a scroll or a wand -- with one difference worth saying up front, and a fork partway through depending on which of the two you're building.


A move is not an item
----------------------

Every other thing you can pick up in roog is an entity: it has a `Position` on the floor, an `Item` marker, a pack slot, and a fate when you drop or throw it. A move has none of that. It is never spawned, never lies on a floor, and cannot be dropped or thrown -- there is nothing to drop.

A move lives as a bare [`MoveEffect`](../../models/src/components.rs) value inside a creature's `Moveset` component:

    pub struct Moveset {
        pub slots: Vec<MoveEffect>,
    }

Today only the player has one, taught in play by heroic mana (an uncommon coin-table row that puts a random, not-yet-known move straight into the taker's `Moveset`, up to `constants::moves::MOVESET_CAP` -- four) rather than started with any. Giving a move to the player for testing without going through the drop table means putting it in that `Vec` by hand -- there is no `ROOG_SPAWN` for this, because there is no entity to spawn. See "Hold it in your hands" below.

The other consequence of not being an item: a move costs [`Magic`](../../models/src/components.rs), not a battery. `MoveDef::cost` is spent by [`move_system`](../../models/src/items/moves.rs) every time the *player* triggers it. A monster that happens to do the same trick under its own steam -- the dragon's fireball is the one example today -- does not go anywhere near `Moveset`, `MoveQueue` or a Magic cost; it is wired straight into `crate::ai` and `crate::items::dragon_breath`, and pays nothing. A `MoveEffect` variant is a shared *mechanic*, not a shared *economy*.


The recipe
----------

Two steps are the same whatever you're building; then the path forks on `MoveKind`.

1. **Append** a variant to `MoveEffect` in `models/src/components.rs` -- at the end, never in the middle (see the warning below):

       pub enum MoveEffect {
           DragonBreath,
           // ... fourteen more ...
           HasteSelf,
           IceBolt,
       }

2. **Add the row** in `MOVES` (`models/src/catalog.rs`):

       MoveDef { effect: MoveEffect::IceBolt, name: "Ice Bolt", cost: 2, range: 6, kind: MoveKind::Attack },

   `cost` is Magic points, doubled by a staff (`TurboMagic`) when `kind` is `MoveKind::Attack`, never for a `MoveKind::Skill`. `cost` is also calibrated against `constants::player::START_MAGIC` (4) and sits in one of four Magic-cost tiers, 1 through 4 -- see MANUAL.md, "Moves", for where the existing sixteen land. `range` feeds the aiming reticle exactly like a wand's `Ranged`, and is meaningless -- leave it `0` -- for a move you're about to make a Skill.

Which of the two you're building decides everything from here.


### Create an attack

An `Attack` opens the aiming reticle, deals damage (or a status a target can shrug off, like Thunderbolt's paralyse), and is the one `TurboMagic` doubles both the cost and the fury of.

3. **Write the mechanic** as one arm of `apply_move_effect` (`models/src/items/moves.rs`):

       fn apply_move_effect(world: &mut World, user: Entity, target: Position, effect: MoveEffect, power_mult: i32) {
           match effect {
               MoveEffect::DragonBreath => breathe_fire(world, user, target, power_mult),
               MoveEffect::IceBolt => ice_bolt(world, user, target, power_mult),
           }
       }

   The match has no catch-all, the same as `apply_wand_effect` over `WandEffect` -- step 1's new variant will not build until this step gives it an arm. The compiler names the function and the missing variant, so there is nothing to remember. `power_mult` is `2` when a staff doubled the cost, `1` otherwise -- fold it into your damage roll (multiply the final number, the way every existing `Attack` arm does; see `sting`, `thunderbolt`, `force_lance` for the shape when armour is or isn't subtracted).

   Most of an attack's mechanic is borrowed rather than written:

     * **A blast at a point** -- `breathe_fire`, `lux`, `meteor_strike` all call `elemental_blast` (`items/wands.rs`), the same function a zapped or thrown wand calls. Pick a radius (`BLAST_RADIUS` for a zap-sized disc, `GRENADE_RADIUS` for a thrown-sized one), a damage roll, and an `Element` (or `None` for non-elemental).
     * **A line** -- `force_lance` traces `get_line(user_pos, target)` by hand, stopping at the first wall, and rolls damage per creature it crosses. Copy it when the flavour is "a bolt" or "a beam" rather than "an explosion".
     * **A single target hit like a trap or a wand bolt** -- `sting` (the dart trap's own formula, aimed rather than laid) and `thunderbolt` (a plain armour-ignoring roll, like a wand bolt) both just call `crate::helpers::monster_at(world, target)` and roll once.
     * **Everything in view, no target needed** -- `circle_of_death` and `frost_nova` call `crate::helpers::hostiles_in_view(world, user)` instead of aiming at a point; pair this shape with step 4's "no target" answer below.

   Whichever shape you pick, check the target for `MagicWard` before you roll and call `ward_block(world, victim)` (or route the damage through `elemental_blast`, which already checks it in `wands::damage_with_element`) -- see "Play fair with Magic Ward" below.

4. **Say whether it needs a target.** Every attack above that hits one aimed tile does; `MoveEffect::needs_target` (`models/src/components.rs`) lists the ones that don't, because they hit everyone in view instead (Circle of Death, Frost Nova) -- add your variant there if that's the shape you picked in step 3, the same way `WandEffect::needs_target` carves the wand of light out from every other wand. Leave it out (needing a target is the default) for anything aimed.


### Create a skill

A `Skill` never opens on damage. It changes the caster (Cure, Heal, Bide, Magic Ward, Haste Self), the floor around them (Setup), or borrows a scroll's own effect outright (Identify, Magic Mapping) -- and `TurboMagic` never touches its cost.

3. **Decide if it needs a target at all.** Almost none do -- a skill works on the caster or on everyone in view, the same as a room-wide attack. Add your variant to `MoveEffect::needs_target`'s `false` list (`models/src/components.rs`) unless you are building the rare aimed skill (nothing in the game does this yet; if you do, treat `target` as step 3 of "Create an attack" would).

4. **Write the mechanic**, the same match arm as an attack, but built one of two ways:

     * **Borrow a scroll wholesale.** Identify and Magic Mapping are one line each:

           MoveEffect::Identify => super::scrolls::apply_scroll_effect(world, user, ScrollEffect::Identify),

       Reach for this whenever an existing scroll already does exactly what you want -- a move is a delivery method, not a reason to write the mechanic twice.

     * **Write it fresh**, reaching into `crate::conditions` for anything that changes the caster's state (`cure_one_condition`, `hasten`, `paralyse`, ...) the way `cure_self` and `haste_self` do, or inserting a plain marker component the way `magic_ward` inserts `MagicWard` and `bide` inserts `Bided`. A skill that plants something on the floor (Setup) reaches into `crate::traps` instead -- see `setup` for spawning a revealed trap and springing it immediately if something is already standing there.

   `power_mult` is still a parameter every arm takes, by convention (every arm in the match has the same signature) -- a `Skill` is free to ignore it, since `move_system` never doubles a `Skill`'s cost.


### Play fair with Magic Ward

Any damage or affliction your move deals to another creature should check `MagicWard` first, the same way every existing damaging move does:

    if world.get::<MagicWard>(victim).is_some() {
        ward_block(world, victim);   // logs the line, plays the ricochet
        return; // or `continue`, in a loop over several victims
    }

Routing through `elemental_blast` gets this for free (it's checked once, centrally, in `wands::damage_with_element`); anything that rolls damage by hand -- `sting`, `thunderbolt`, `force_lance`, `circle_of_death`, `frost_nova` -- checks it itself, per victim, before the roll. Skip this and your move becomes the one hole in an otherwise complete shield, which is a worse bug than it sounds: the player *bought* that immunity with three whole Magic points.


Hold it in your hands
----------------------

There is no `ROOG_SPAWN` for a move. In play the only way to learn one is heroic mana (see "A move is not an item" above); to try Ice Bolt without hunting for mana, replace the player's starting `Moveset::default()`, in `initialize_world` (`models/src/map/levels.rs`), with:

    Moveset {
        slots: vec![MoveEffect::IceBolt],
    },

Build, run, and press `Z` to see it listed, or `Alt`+`W` to aim it directly -- `Alt`+`Q`/`W`/`E`/`R` fire slots 1-4 in order. `Tab` while aiming snaps the reticle to the next thing in view.

If this was a dry run, `git checkout models/src/map/levels.rs` along with `catalog.rs`, `components.rs` and `items/moves.rs`.


> **Append enum variants; never insert or reorder them.** `MoveEffect` derives `Serialize`, and a save encodes a variant as its position in the enum -- including inside a saved `Moveset` (see the next warning). Adding one at the end is safe; putting one in the middle silently turns a saved move into a different one, for every save that had it.

> **`Moveset` does survive a save** -- `models/src/saveload.rs` round-trips it like every other piece of the player. So do the two marker components moves have left behind so far, `MagicWard` and `Bided` -- a bare `bool` field each in `EntitySave`, same shape as `confused` or `confusing_touch`. If your new move leaves a marker of its own lying around outside `Moveset` itself, give it the same treatment or a reload will quietly forget it.

> **Nothing routes a move onto a monster generically.** A `MoveEffect`'s mechanic is shared; the trigger is not. Wiring a species to use one under its own AI, at no Magic cost, is bespoke work in `crate::ai` -- read how `FireBreath` and `dragon_breath` do it for the dragon before assuming a row anywhere makes this automatic.


Verify what you added
----------------------

    cargo build

There is no `cargo test --test content` coverage for moves the way there is for monsters and items -- `MOVES` is not part of `spawn_named` or `content_names()`, because nothing about a move is spawned. Prove it by hand: give it to the player, aim it, and watch the log line `move_system` prints (`"You focus, and unleash your ___!"`) followed by whatever `apply_move_effect` logs itself.


See also
--------

  ../tutorial/add-your-first-move.md   the long version, as a lesson
  add-an-item.md                       the pattern this one borrows
  ../reference/input-and-turn-loop.md  where `move_system` sits in the schedule
