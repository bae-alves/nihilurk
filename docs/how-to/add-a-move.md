How to add a move
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-move.md` first.
    Result         A new active ability the player can trigger with
                   `Z` or `Alt`+a letter, that costs Magic and aims
                   like a wand.

One enum variant, one catalog row, one arm of an exhaustive match. The same shape as a potion, a scroll or a wand -- with one difference worth saying up front.


A move is not an item
----------------------

Every other thing you can pick up in roog is an entity: it has a `Position` on the floor, an `Item` marker, a pack slot, and a fate when you drop or throw it. A move has none of that. It is never spawned, never lies on a floor, and cannot be dropped or thrown -- there is nothing to drop.

A move lives as a bare [`MoveEffect`](../../models/src/components.rs) value inside a creature's `Moveset` component:

    pub struct Moveset {
        pub slots: Vec<MoveEffect>,
    }

Today only the player has one. Giving a move to the player for testing means putting it in that `Vec` by hand -- there is no `ROOG_SPAWN` for this, because there is no entity to spawn. See "Hold it in your hands" below.

The other consequence of not being an item: a move costs [`Magic`](../../models/src/components.rs), not a battery. `MoveDef::cost` is spent by [`move_system`](../../models/src/items/moves.rs) every time the *player* triggers it. A monster that happens to do the same trick under its own steam -- the dragon's fireball is the one example today -- does not go anywhere near `Moveset`, `MoveQueue` or a Magic cost; it is wired straight into `crate::ai` and `crate::items::dragon_breath`, and pays nothing. A `MoveEffect` variant is a shared *mechanic*, not a shared *economy*.


The recipe
----------

1. **Append** a variant to `MoveEffect` in `models/src/components.rs`:

       pub enum MoveEffect {
           DragonBreath,
           FrostNova,   // <- at the end. See the warning below.
       }

2. **Add the row** in `MOVES` (`models/src/catalog.rs`):

       MoveDef { effect: MoveEffect::FrostNova, name: "Frost Nova", cost: 2, range: 6 },

   `cost` is Magic points; `range` feeds the aiming reticle, exactly like a wand's `Ranged`.

3. **Write the mechanic** as one arm of `apply_move_effect` (`models/src/items/moves.rs`):

       fn apply_move_effect(world: &mut World, user: Entity, target: Position, effect: MoveEffect) {
           match effect {
               MoveEffect::DragonBreath => breathe_fire(world, user, target),
               MoveEffect::FrostNova => frost_nova(world, user, target),
           }
       }

   The match has no catch-all, the same as `apply_wand_effect` over `WandEffect` -- step 1's new variant will not build until step 3 gives it an arm. The compiler names the function and the missing variant, so there is nothing to remember.

   Most of a move's mechanic is borrowed rather than written. `breathe_fire` is `elemental_blast` (`items/wands.rs`) with the damage computed from the caster's own `Fighter` and `Loadout` instead of a wand's dice -- see it for the shape a damaging move takes; a status-effect move would instead borrow from `crate::conditions` the way a wand's utility effects do.

4. **Give it a range and a cost that mean something.** `range` is a straight lift from the closest wand: 6-8 is normal, `THROW_RANGE` (an arm's length) is the ceiling. `cost` is calibrated against `constants::player::START_MAGIC` (4) -- Fireball's 2 lets a fresh player cast it twice.


Hold it in your hands
----------------------

There is no `ROOG_SPAWN` for a move. To try one, put it in the player's starting `Moveset`, in `initialize_world` (`models/src/map/levels.rs`):

    Moveset {
        slots: vec![MoveEffect::DragonBreath, MoveEffect::FrostNova],
    },

Build, run, and press `Z` to see it listed, or `Alt`+`W` to aim it directly -- `Alt`+`Q`/`W`/`E`/`R` fire slots 1-4 in order. `Tab` while aiming snaps the reticle to the next thing in view.

If this was a dry run, `git checkout models/src/map/levels.rs` along with `catalog.rs`, `components.rs` and `items/moves.rs`.


> **Append enum variants; never insert or reorder them.** `MoveEffect` derives `Serialize`, and a save encodes a variant as its position in the enum. Adding one at the end is safe; putting one in the middle silently turns a saved move into a different one. (A run's `Moveset` is not saved today -- see the warning below -- but the enum follows the same rule as every other saved effect key in the codebase, against the day it is.)

> **A `Moveset` does not survive a save yet.** Every other piece of the player -- HP, Magic, the pack -- round-trips through `models/src/saveload.rs`; `Moveset` does not have an entry there. A reload restores whatever `initialize_world` or `load_game` puts on the player, not what the run had equipped. If you are wiring a second move for real rather than for testing, add `Moveset` to the save format first.

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
