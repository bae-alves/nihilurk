Reference: rendering
=====================

    Audience       Engine developers only. There is no table here —
                   this describes how a frame is painted, not what
                   appears in it.
    Prerequisites  A passing familiarity with bevy_ecs and
                   `crossterm`, and `reference/components.md` for the
                   resources read below (`BloodStains`, `Corpses`,
                   `Smoke`, `Particles`, `MagicMapReveal`, `AnimRate`).
    Status         Describes `engine/src/view.rs` as it is in the
                   source. If this page and the source disagree, the
                   source is right and this page is a bug.

**Higher-risk ground than a content table.** `engine/` carries almost
no automated tests, and `view.rs` has none at all — it is checked by
looking at the terminal. Two things make a change here easy to get
subtly wrong: the draw order in `render` (a later layer silently
covers an earlier one), and `Screen`'s shape being hand-duplicated in
`perf/src/screen.rs` for the performance-testing rig — see "The frame
buffer" below. Change it, then run the game and look, in more than one
terminal if you can.


The frame buffer
-----------------

```
pub struct Screen {
    cur: Vec<Cell>,   // this frame, being painted
    prev: Vec<Cell>,  // last frame, as flushed
    ...
}
type Cell = (char, Color, Color); // glyph, foreground, background
```

`render` paints the entire 80x25 grid into `cur` every call — nothing
is incremental at that level. `Screen::flush` is what's incremental:
it diffs `cur` against `prev` cell by cell and only emits a
`MoveTo`/`Print` (plus `SetForegroundColor`/`SetBackgroundColor` when
the colour actually changed) for cells that differ, then swaps the two
buffers. A typical turn changes a handful of cells; `flush` writes a
handful of escape sequences, not 2000. `dirty_all` forces a full
repaint — set on the first frame, and whenever the centering offset
changes (`-c`, or the terminal was resized), since a shifted viewport
would otherwise leave stale cells sitting where the old frame was.

`put`/`set_fg`/`set_bg`/`puts`/`hline` are the only ways `render`
touches `cur`; they silently no-op on an out-of-bounds coordinate
rather than panicking, since a couple of callers (the targeting beam,
the travel cursor) compute a tile position that can legitimately fall
off the 80x25 frame.

`centering_offset` reads `RenderConfig.centered` (`-c`) and the real
terminal size to compute the top-left offset that keeps the fixed
80x25 frame centred; `(0, 0)` when not centred.

**This struct is manually duplicated.** `engine` is a binary crate
with no library target, so the performance-testing rig
(`perf/src/screen.rs`) cannot depend on it and instead reimplements
`Screen` from scratch to measure flush cost in isolation. Nothing
enforces that the two stay in step — see
`../explanation/performance-testing.md`, "The grid is a copy, and it
can drift". If you change `Screen`'s public shape or its diffing
logic, check that file's copy by hand.


`render`: the layers, in order
--------------------------------

```
pub fn render<W: Write>(world, stdout, screen) -> std::io::Result<()>
```

Painted in this order — everything after "Terrain" draws over
whatever came before it on the same cell:

  1. **Top HUD** (row 0) — see below.
  2. **Terrain** — every tile with `visible` or `revealed` set;
     `revealed`-but-not-`visible` tiles are painted `DarkGrey` (the
     "remembered, not seen" fog look). A corridor's walls are skipped
     entirely (`Map::is_room_wall`) so passages read as tunnels, not
     ditches framed on every side.
  3. **Blood overlay** — recolours a tile's existing glyph
     (`set_fg`, not `put`) `DarkRed`, only where currently `visible`
     and never where `occupied_by_actor` — the stain marks the floor,
     not whatever is standing on it.
  4. **Corpses** — same visibility rule as blood, drawn as a `%`.
  5. **Floor items** — visible and not actor-occupied.
  6. **Traps** — the one exception to "unseen means blank": a
     previously-discovered trap (`Without<Hidden>`) still shows in
     `DarkGrey` once out of sight, same as a known staircase.
  7. **Smoke** — visible, not actor-occupied, `≈` in grey; fades on
     its own over a few turns (see `Smoke` in components.md).
  8. **Actors** — the player and every non-`Hidden` `Mob`, visible
     only.
  9. **Particles** — drawn over actors deliberately, so a hit motes
     over the thing it hit rather than under it.
  10. **Targeting beam** — a Bresenham line from the player to the
      reticle, drawn as `*` in yellow, except where it crosses an
      actor: the actor's own glyph is kept but recoloured yellow (or
      black, if the actor was already yellow-ish, so it doesn't
      vanish into the beam). The reticle's own tip additionally gets a
      `DarkBlue` background.
  11. **Travel cursor** — a background-only highlight (`set_bg`), so
      the glyph and colour of whatever's on that tile stay readable.
  12. **Message log** (rows 22–24).
  13. **Inventory overlay** — drawn last, on top of everything.

`occupied_by_actor`, computed once up front, is the set every "don't
draw under a mob" rule in steps 3–8 checks against.


The HUD
--------

Built as an ordered list of fields — name, `HP x/y`, `Ma x/y`, `Pow.`,
`Arm.`, optionally `Thr.`, `DEPTH n` — joined with `" · "` in
`DarkGrey`, then either a `SCORE` field or, if any transient condition
badge is lit, the badges in its place (there's only room for one).

The displayed `Pow.`/`Arm.`/`Thr.` figures are **not** just
`Fighter.power` etc. — they fold in every equipped modifier via
`equipped_total::<PowerDie>` and friends, the same fold `combat_system`
runs, so the HUD can never drift from the number combat actually rolls
against. `Thr.` only appears once it's nonzero, since a player who
never picked up something that boosts throws never needs to see a
field that would always read `+0`.

Condition badges, in the order checked: `FAST`/`SLOW` (`Speed.kind`),
`CONF` (`Confused`), then a snare label (`HELD` for a bear trap,
`ASLEEP` for sleeping gas), then an auto-walk badge
(`EXPLORING`/`TRAVELING`, or `ASCENDING` — magenta — once the player
carries the Element of Yoord), then `TRAVEL?` while the `O` cursor is
open. Each is independent; several can show at once.


Animation playback
--------------------

Both of these are **blocking** loops called once per main-loop
iteration, after the schedule runs and before the frame's own
`render`. Both are no-ops when nothing armed them, so a turn that
fought nothing and read no scroll passes straight through.

```
pub fn play_particles<W: Write>(world, stdout, screen) -> std::io::Result<()>
pub fn play_magic_map<W: Write>(world, stdout, screen) -> std::io::Result<()>
```

Both share the same shape: while the effect (`Particles`/
`MagicMapReveal`) still has something to show, advance it one frame,
call `render` to paint the result, then `poll` for the frame's
duration — a keypress during that poll is swallowed and skips straight
to the finished state (magic mapping additionally
`finish_magic_map_reveal`s the map instantly rather than leaving it
part-revealed). Frame duration is `AnimRate`-scaled (`-anim-rate`,
clamped `0.1..=5.0`) off a `33ms` base for particles and
`MagicMapReveal::frame_ms()` for the map wipe, so a slow terminal or a
player who wants snappier turns can retune both without either one's
code changing.

The turn itself is already fully resolved by the time either of these
runs — they only animate what already happened, which is what makes
it safe to block input here the way NetHack and DCSS do for a bolt.


End-of-run panels
-------------------

```
pub fn render_you_died<W>(stdout, screen, offset) -> std::io::Result<()>
pub fn render_tombstone<W>(stdout, screen, offset, player_name, cause, score)
pub fn render_victory<W>(stdout, screen, offset, player_name, score)
```

Full-screen, drawn in place of `render` (not layered with it) via
`screen.clear()` and `screen.dirty_all = true` — a full repaint, since
nothing about the map frame should bleed through. `centered_x` centres
a line of text; `wrap_words` (victory's blessing line only) greedily
wraps on word boundaries, never splitting a word, so a long player
name can't run the line off the panel. `main.rs`'s
`run_death_screens`/`run_victory_screens` drive these: show the
"You die..."/starfield panel, block for the acknowledging keypress
(`wait_for_key`), then (death only) show the tombstone and block again.


The inventory overlay
------------------------

```
fn draw_inventory(world: &mut World, screen: &mut Screen)
```

A bordered box sized to the widest row actually being drawn
(`row_text`, the one place a row's text — letter, name, `" (E)"` for
equipped — is assembled, so sizing and drawing can never disagree
about a row's width) or 30 columns, whichever is larger. Row colour is
a small decision table on `(selected, cursed_known, equipped)` —
`known_quality` gates the cursed colouring, so an unidentified cursed
item still reads as ordinary. The Use/Throw/Drop action modal, when
open, is a second small box floated to the right of the item row it
belongs to, its three labels coming from `ActionMenu::actions()` in
whatever order `-dropthrow` put them in.


See also
--------

  input-and-turn-loop.md        the other half of the frame: keyboard and turns
  ../reference/components.md    the resources named throughout this page
  ../reference/cli-and-env.md   `-c`, `-anim-rate`, `-nb`
  ../explanation/performance-testing.md   how this file is benchmarked, and the `perf/src/screen.rs` copy
