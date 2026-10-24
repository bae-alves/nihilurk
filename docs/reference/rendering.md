Reference: rendering
=====================

    Audience       Engine developers only. There is no table here —
                   this describes how a frame is painted, not what
                   appears in it.
    Prerequisites  A passing familiarity with bevy_ecs, and
                   `models/src/meta/components/` for the
                   resources `view.rs` reads.
    Status         Describes `engine/src/view.rs` and
                   `engine/src/screen.rs` as they are in the source.
                   If this page and the source disagree, the source is
                   right and this page is a bug.

**Nothing on this page is covered by an automated test, and that is the policy rather than a gap.** A test of the renderer, the HUD or the frame geometry would check which glyph lands in which cell, so it would break whenever a layer moved and never catch something a glance at the terminal would miss. The reasoning is `../explanation/adr-0004-no-tests-of-presentation.md`. The easiest thing to get wrong here, a later layer silently covering an earlier one, is caught by you: change it, run the game, and look.

The one thing asserted is not presentation. `log_panel`, which holds the `--MORE--` prompt back while particles play, is pinned by `a_backlog_holds_its_prompt_until_the_animation_is_over` in `view.rs`. It protects a keypress, and the frame it is wrong in looks exactly right.


The frame buffer
-----------------

`Screen` (`engine/src/screen.rs`) holds the grid, a double buffer and the cell diff, and knows nothing about the game. `render` paints the whole grid every call; `Screen::changed` yields only the cells that differ from the last frame, and `Term::present` (`engine/src/term.rs`, the one file that names crossterm) writes those and swaps the buffers. `dirty_all` forces a full repaint, on the first frame and whenever the centering offset (`-c`) changes.

**Two coordinate systems.** `put`/`puts`/`hline` take *screen* coordinates: the HUD, the log and the overlays. `put_map`/`fg_map`/`bg_map` take *map* coordinates and go through `Screen::map_cell`, which adds `MAP_TOP` and the shake's `map_shift`, then clips. It never clamps: a tile shaken off the edge is not drawn. Every painter no-ops out of bounds rather than panicking. Nothing above `term.rs` sees the centering offset; a click arrives already in grid coordinates.


`render`: the layers, in order
--------------------------------

Each layer draws over the ones before it on the same cell. The order is the code in `render`; this list is a map of it.

  1. Top HUD (row 0, screen coordinates)
  2. Terrain, with the stealth-range and special-room tints
  3. Blood
  4. Corpses
  5. Floor items
  6. Traps (a found trap stays drawn out of sight, like a known staircase)
  7. Smoke
  8. Actors
  9. Monster status tints (`status_tint`), then the trick-shot magenta, which outranks them
  10. Detected things out of sight
  11. Particles, over actors on purpose
  12. Targeting beam and blast preview (`models::aim_footprint`)
  13. Travel cursor
  14. Player line, score flash, balance gauge, command bar, message log (screen coordinates)
  15. Overlays, last: pack, spells, offer, barter, F1 key list, Info popup, quit prompt

Layers 2–13 paint in map coordinates, so the shake moves them. **Blood, corpses, items and smoke skip any tile in `occupied_by_actor`**: the stain marks the floor, not whoever stands on it. **Blindness takes the colour out of the map**: map colours go through `by_touch`, which returns white, except remembered tiles and detected things, which the player is not looking at.


The HUD
--------

Row 0 is the dungeon's half: condition badges from the left, `DEPTH` and `TIME` centred, pack and score on the right. **The three anchors do not negotiate**, so a long badge run can paint over `DEPTH`, and that is accepted: the alternative is dropping a badge the player needs. The badges, and the order they show in, are built in `render`.

Row 22 is the player's half. **Every figure comes from the same call combat uses**, so the HUD cannot drift from the game: `Pow.`/`Arm.`/`Skl.` from one `models::loadout` fold, and `HP` turns red on `models::player_too_injured`, the same check auto-fight refuses under.

**Priority badges are never shed.** A creature carries at most `CONDITION_CAP` ledger conditions (`models/src/combat/effects/ledger.rs`), but the tempo badges, a worn ring's `STLH` and the two coin promises sit outside that count. Each is something the player cannot act correctly without seeing.

`score_text` truncates and never rounds, so a score never reads higher than it is.


Animation playback
--------------------

The main loop calls `play_particles` and `play_magic_map` after the schedule and before the frame's own `render`. Both **block**: the turn is already resolved, so freezing input to show it is safe, and a keypress skips to the end and is swallowed. `drain_input` empties the queue before the first frame and `interrupted` counts only key presses, so the release of the key that fired a wand cannot kill its own bolt.

**`--MORE--` waits for the particles** (`log_panel`). The prompt asks for a key, and any key skips the animation, so a prompt shown mid-burst would eat a trick shot's blast. Only the prompt waits; the log lines still paint.

`play_shake` is the odd one out: it **never blocks**. The dungeon arms a shake at the moments a player is most likely typing ahead, so the moment a key arrives it settles the map, repaints and returns *without reading the key*. It runs straight after the frame that armed it, so the map is never left skewed while the loop waits. `-nshake` stops anything arming one (`Shake::enabled`); `rg ShakeKind::` finds what does. The blocking loops age the shake too, so a blast's shake and sparks play together and skip together.

Frame timing for all three scales with `AnimRate` (`-anim-rate`).


End-of-run panels and overlays
-------------------------------

`render_you_died`, `render_lose` and `render_win` replace `render` with a cleared, fully dirty screen; `main.rs`'s `run_death_screens` and `run_victory_screens` drive them. The run dump (`morgue.rs`) is written first, while the last map frame is still on screen, so `Screen::shown_text` captures what the player last saw.

`draw_inventory` takes its rows from `models::pack_rows` and its title from `PackMode::title`, so it can never show a row the cursor cannot reach. A row's letter is its backpack index; its screen line is its place in the filtered list.

`draw_quit_prompt` paints last, centred over everything: it is the one modal whose job is to slow the player down.


See also
--------

  input-and-turn-loop.md        the other half of the frame: keyboard and turns
  ../explanation/adr-0004-no-tests-of-presentation.md   why none of this is unit-tested
  ../../models/src/meta/components/  the resources `view.rs` reads
  ../reference/cli-and-env.md   `-c`, `-anim-rate`, `-nb`, `-nshake`
