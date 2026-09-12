Reference: input handling and the turn loop
============================================

    Audience       Engine developers only. There is no table here, no
                   row to copy — this is the engine itself: the main
                   loop, the keyboard, the modal stack.
    Prerequisites  A passing familiarity with bevy_ecs, and
                   `reference/components.md` for the resources named
                   below (`GameLog`, `PackIsOpen`, `TargetingState`,
                   `AutoExplore`, `FastMove`, `TravelCursor`, `Ending`).
    Status         Describes `engine/src/update.rs` and the main loop
                   in `engine/src/main.rs` as they are in the source.
                   If this page and the source disagree, the source is
                   right and this page is a bug.

**This is higher-risk ground than a content table.** A wrong row in
`models/src/monsters.rs` is a monster with too much HP; a wrong branch
here is a keypress that eats a turn it shouldn't, a modal that can't be
closed, or a run loop that spins. `engine/` has almost no test
coverage of its own — `update.rs`'s two numpad tests, below, are it —
so a mistake is far more likely to be caught by a person playing the
game than by `cargo test`. Change this file by reading it end to end
first, and test by hand: walk into a wall confused, open the pack
mid-throw, run a Shift-direction into a monster.


The split this code sits on
----------------------------

`models/` holds the pathfinding and state machines (`autoexplore.rs`,
`fastmove.rs`) as **pure functions that only read the world** — no
terminal, no sleeping, no turn-taking. `engine/src/update.rs` is the
other half: it owns the keyboard (`crossterm::event::read`/`poll`),
decides when a turn is spent, and drives those pure functions to
completion. Nothing in `models/` blocks on input or sleeps; nothing in
`engine/` does pathfinding. If you find yourself wanting to add a
`World` query that decides *where* to go, it almost certainly belongs
in `models/`, not here — see the module doc comments at the top of
`autoexplore.rs` and `fastmove.rs`.


The main loop
-------------

    engine/src/main.rs, fn main

One `Schedule`, run once per turn, in this fixed order:

    smoke_system -> snare_system -> passive_ability_system -> ai
      -> trap_system -> throw_system -> item_system
      -> equipment_effects_system -> combat_system -> reaper_system
      -> dungeon_lord_system -> visibility_system

Each pass through `while world.resource::<GameState>().is_running`:

  1. **Advance the game.** A fast-move run in progress
     (`FastMove::active`) resolves entirely inside `fast_move_run` —
     it steps the schedule itself, turn by turn, without repainting.
     Otherwise, one player step is taken (see `player_step` below);
     if it consumed a turn, `schedule.run` fires once.
  2. **Play any queued animation.** `view::play_particles` then
     `view::play_magic_map` — both no-ops unless something armed
     them this turn. Both **block**: input is frozen for the length
     of the effect, and a keypress skips to the end and is swallowed.
  3. **Render.** `view::render`.
  4. **Let the screen shake settle.** `view::play_shake`, and this one
     is the odd step out: it does **not** block. It animates only while
     nothing is waiting to be read, and the moment a key arrives it
     settles the map and returns *leaving the key unread* for step 1 of
     the next pass. That is also what guarantees the map is never left
     skewed while the loop blocks — the shake either runs out here or is
     settled here. A no-op unless a turn armed one (and `-nshake` means
     none ever is); see `rendering.md`, "The screen shake".
  5. **Pace it.** A 35ms sleep per step while auto-exploring, a 90ms
     sleep per turn while the player is incapacitated (asleep in gas),
     so both read as time passing rather than a freeze or a blur.
  6. **Check the ending.** `Ending::player_dead` or `player_won`
     breaks the loop into the death or victory screens.

```
fn player_step(world: &mut World) -> std::io::Result<bool>
```

One player-side step, in priority order: an `AutoExplore` tick, else
a `TravelCursor` tick, else a blocking read of the player's own move
via `process_input_and_update`. Returns whether a turn was spent —
the main loop only runs the schedule (lets monsters act) when this is
`true`.


The modal stack
----------------

```
pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool>
```

Before reading a key at all, two things can consume the frame without
one: a pending `--MORE--` prompt (only Space/Enter clears it; every
other key is swallowed) and `player_incapacitated` (asleep in gas —
the turn is forfeited outright with no key read; a bear trap does
*not* forfeit the turn, since it only blocks movement, not the whole
turn — see "Movement and attack" below).

Once a key is read, `x` and `X` are checked first, everywhere: they are
the universal escape hatch, closing whichever modal is open
(`close_all_modals`, the quit prompt included) and returning to plain
movement with no turn spent. `X` is checked *here*, rather than next to
`Q` in `handle_movement_input`, precisely so that it escapes rather than
asks about ending the run — it is one shift away from `x`, and only
falls through to the quit prompt when `close_all_modals` reports there
was nothing to close. After that, exactly one of four contexts owns the
keypress:

| Context (checked in this order) | Resource      | Handler                    |
|----------------------------------|---------------|-----------------------------|
| "Really quit?"                   | `QuitPrompt.open` | `answer_quit_prompt`   |
| Aiming (a throw or a ranged use)| `TargetingState.active` | `handle_targeting_input` |
| Pack open                        | `PackIsOpen.open` | `handle_inventory_input` |
| Walking the map                  | (default)     | `handle_movement_input`    |

They nest one level deep only: the pack can open the aiming reticle
(`Throw`, or a `Use` that needs a target), but the reticle itself has
no modal under it. The quit prompt is raised only from the map, so it
never has anything under it either — it is first in the list because
while it is up, that question is the only one on the table. `y` quits,
`n` / `Esc` cancels (and so do `x` / `X`, upstream), every other key is
ignored rather than guessed at.


Movement and attack
--------------------

```
fn move_player(world: &mut World, dx: i16, dy: i16) -> bool
```

The single path every step and every melee attack goes through
(auto-explore, fast-move and the plain arrow keys all call this).
Checked in order, each one able to end the attempt:

  1. **Confusion stumble** (`maybe_stumble`) — while `Confused`, a
     coin flip hijacks the step into one of the eight
     `STUMBLE_DIRS` at random, logging "You stumble foolishly." A
     stumble into a wall still burns the turn; a deliberate wall-bump
     does not.
  2. **Wall.** `Map::blocks`.
  3. **Diagonal cut.** `Map::diagonal_step_ok` — a diagonal step must
     connect two tiles of the same kind, so you can't cut a doorway
     corner or squeeze from a corridor into a room diagonally.
  4. **A `Mob` on the target tile** — attacks instead of moving
     (`player_attack`, which is `resolve_attack`, the same opposed-roll
     resolution monsters use).
  5. **A bear trap holding the player** (`SnareKind::Bear`) — an
     adjacent swing above still lands as an attack (step 4), but a
     plain step becomes `bear_trap_thrash`: a wasted turn, a scratch
     of damage, blood. See `docs/*traps*` for the trap itself.
  6. **The move.** Position updates, the player's `Viewshed` is
     marked dirty, and `EntityMoved` is tagged on the player so
     `trap_system` checks the new tile.
  7. **Pickup.** An `Item` on the landed tile is stowed
     (`models::stow`) — which can merge into an existing quiver stack,
     leave part of a pile behind if the pack is full, or refuse
     outright ("Your pack is full."). A `Hidden` (invisibly stashed)
     item announces itself the instant it's stepped on.

Every one of steps 2–5 can return early; only reaching the move at
step 6 (or a hijacked stumble into a wall) consumes a turn.


The keyboard on the map
------------------------

Movement is vi keys, arrows and the numpad, eight ways, plus
Shift+direction to run (`run_direction`). **WASD is not a movement
scheme any more**, shifted or otherwise: those letters are commands.

**`Esc` does not quit.** It is the key a player mashes to get out of a
menu; from the map it now does nothing at all. Quitting is `Q` or `X`
through the prompt, or Ctrl+C without one.

| Key | `handle_movement_input` does |
|-----|------------------------------|
| `i` `a` `t` `d` `e` `q` `r` `w` `W` `P` | `open_pack(world, PackMode::…)` — see below |
| `o` / `O` | auto-explore / travel cursor |
| `A` | `toggle_auto_pickup` — flips `AutoPickup::enabled`, logs which way it landed, spends no turn |
| `f` / `Tab` | fire the wielded launcher / auto-fight |
| `>` `.` / `<` `,` | stairs, or travel to them |
| `Q` / `X` | raise `QuitPrompt` — the "Really quit?" modal. `X` only reaches here with nothing open; otherwise it is the escape hatch above |
| Ctrl+C | clear `GameState::is_running` on the spot, no prompt |

Adding a command key is a row in that `match` and (if it opens the pack)
a row in `PackMode`. Check it against the pack's own letters first: the
item rows are `a`..`i` (`PACK_CAPACITY` is 9), and `navigate_pack`'s
letter arm claims every lowercase key that isn't already navigation.


The pack and the action modal
-------------------------------

Ten keys open the pack, each in a `PackMode` (`models/src/pack.rs`) that
decides the title, which rows are shown (`pack_rows`) and what picking
one does (`PackMode::action`). `i` is the only one that asks afterwards;
the rest carry their own verb and commit on the spot, closing the pack
as they go. `open_pack` refuses with the mode's own line ("You have
nothing to read.") rather than opening an empty box.

Rows are **backpack indices**, not row numbers, everywhere —
`PackIsOpen::selected`, `pack_rows`, `commit_item_action`'s `item_idx`,
and the letter `draw_inventory` paints. That is what keeps an item's
letter the same in every menu; `step_row` walks the cursor between the
admitted indices, skipping the rest.

The action modal (`run_action_modal`, `PackMode::Browse` only) is a fixed
three-row menu — `ItemAction::MENU` — that dispatches to
`commit_item_action`:

| `ItemAction` | What happens | Spends a turn? |
|--------------|--------------|-----------------|
| `Use`        | `use_or_aim`: a plain item queues onto `UseQueue` immediately; a ranged one (has `Ranged`, and isn't a self-targeted wand like light) instead reopens the aiming reticle. | Only the immediate case |
| `Throw`      | `aim_throw`: opens the aiming reticle, unless `throw_refusal` objects (the Element of Yoord, cursed worn gear). | Never here — the throw itself is queued from the reticle |
| `Drop`       | `drop_from_pack`: refused by `drop_refusal` (cursed and equipped) with the item returned to the pack; otherwise unequipped and placed on the floor. | Yes, on success |

`take_pack_item` / `return_to_pack` move an item out of the `Backpack`
and back by index — used so an item earmarked for aiming can sit
"in flight" without being in either the pack or on the floor while the
reticle is up.


The aiming reticle
--------------------

`TargetingState` holds which item, whether it's a throw or a zap, and
the cursor. `move_target_cursor` only allows the cursor onto a tile
that is both currently visible and within `aim_range` — `THROW_RANGE`
for a throw, the item's own `Ranged.range` for a zap, `8` as a
fallback. Confirming (`fire_at_target`) refuses a shot at the player's
own tile ("Great idea! But no.") and otherwise removes the item from
the pack and pushes a `WantsToThrow` or `WantsToUse` onto the matching
queue for `throw_system` / `item_system` to resolve next schedule run.
A throw of a stacked item (arrows) goes through `models::draw_one`
first, which splits one unit off and leaves the rest in the pack slot.


Running, auto-explore, and travel
-----------------------------------

Three engine-owned loops replace a single `process_input_and_update`
call while they're active; each is a thin driver around a pure planner
in `models/` (`fastmove.rs`, `autoexplore.rs`):

| Loop | Armed by | Resource | Planner | Stops on |
|------|----------|----------|---------|----------|
| Fast move ("run") | Shift+direction | `FastMove` | `fast_move_plan`, then `travel_step`/`straight_step` each step | keypress, a monster in view, a message logged, junction/target reached, `FAST_MOVE_STEP_CAP` |
| Auto-explore | `o` | `AutoExplore` (`target: None`) | `explore_step` | same, plus "nowhere left to explore" |
| Travel | `>`/`<` to a known but distant staircase, or the `O` cursor | `AutoExplore` (`target: Some(tile)`) | `travel_step` | same, plus arrival |
| Travel cursor | `O` | `TravelCursor` | (none — it's just a cursor) | Esc/`O`/`x`/`X`, or Enter to commit into a travel `AutoExplore` |

`explore_step` gives a spotted item (see `known_item_tiles`) priority
over frontier exploration outright: while `Item`s without `Hidden`
remain on the floor, it beelines for the nearest one via `first_step`
rather than consulting `AutoExplore::frontier` at all. `move_player`'s
own pickup-on-arrival logic (see above) does the actual stowing; once
the item is gone from the floor, frontier exploration resumes as
before.

`detours_for_loot` is what can call that beeline off, on either of two
counts: the `A` toggle (`AutoPickup::enabled`) is off, or the pack is
already at `PACK_CAPACITY`. The second is not politeness — without it,
a full pack walks to an item, is halted by "Your pack is full.", and
beelines to the same item on the next `o`, so the floor never gets
explored.

Fast move (`fast_move_run`) is the odd one out: it runs its entire
walk **inside one call**, stepping `schedule.run` itself between moves
and never repainting until it returns, so a run reads as a single
jump rather than an animated walk. Auto-explore and travel instead
take one step per `player_step` call (`auto_explore_step`), returning
to the main loop's normal render-and-pace cycle after each — which is
why auto-explore visibly walks while a run visibly jumps.

All three refuse to *start* while confused or with a monster in sight,
and both `auto_explore_step` and `fast_move_run` poll for a pending
keypress before every step and swallow it (`let _ = read()?`) if the
walk needs to abort — otherwise that keypress would also be read as a
move on the next frame.

`travel_cursor_step` is different again: it isn't a walk at all, just
a blinking highlight the player steers with arrow/vi/numpad keys over
already-`revealed` ground (sliding along one axis when the diagonal
neighbour is still unseen), gated to 400ms polls so the blink phase
flips on a timeout. It runs its own loop outside
`process_input_and_update`, so the universal escape hatch never sees its
keys — `x` / `X` are handled in its own cancel arm instead, which is the
only reason closing it works at all. Enter commits the tile to a travel `AutoExplore`
via `nearest_reachable` if the chosen tile isn't itself walkable.


Testing
-------

The only tests in `engine/` live inline in `update.rs`, under
`#[cfg(test)] mod numpad_tests`: that a numpad digit moves identically
to its vi-key equivalent, and that Shift+numpad reads as a run
direction like Shift+arrow. Everything else in this page — the modal
stack, the run/travel/auto-explore state machines, `move_player`'s
ordering — is exercised only by playing the game. What *can* be tested
is tested one level down, in `models/`: `tests/pack.rs` pins what each
menu shows, and `tests/autoexplore.rs` pins when the loot beeline is
called off. When you add a new
branch here, prefer moving any *decision* logic (what should happen)
down into a pure, testable function in `models/`, and leave `engine/`
holding only the parts that must touch the terminal.


See also
--------

  rendering.md                  the other half of the frame: `view.rs`
  ../reference/components.md    the resources named throughout this page
  ../reference/cli-and-env.md   the flags this code reads (`-anim-rate`, `-nshake`)
  ../explanation/code-calisthenics.md   the shape this code (and all engine code) is held to
