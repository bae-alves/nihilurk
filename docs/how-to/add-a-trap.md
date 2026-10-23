How to add a trap
=================

    Audience       Content author.
    Prerequisites  Comfortable adding a row. See
                   `../tutorial/add-your-first-item.md`.
    Result         A new trap the dungeon lays on floors, that reveals
                   itself in the usual three ways, and that survives a
                   save.

Three edits, all in one file, and the compiler enforces the handlers.


The mechanics are one file:

    models/src/traps.rs

The components a trap is built from -- `Trap`, `TrapReveal`, `EntityMoved` -- are nouns and live in `models/src/components.rs` with everything else the ECS is made of. The holds a trap can put on a victim (`Asleep`, `Pinned`, and `Rooted` or `Clamped` if yours wants them) are ordinary effects and live in `models/src/effects.rs`. The row and its handlers stay in `traps.rs`, and no enum names the trap: a `Trap` points at its row.


The recipe
----------

1. **Write the mechanic**, `fn(&mut World, Spring)`:

       fn pit(world: &mut World, spring: Spring) {
           trapdoor_effect(world, spring.victim, spring.is_player, spring.seen);
       }

2. **Say what it looks like**, `fn(&mut World, &'static TrapDef, Position)`:

       fn dust(world: &mut World, _row: &'static TrapDef, at: Position) {
           if let Some(mut fx) = world.get_resource_mut::<Particles>() {
               fx.poof(at.x, at.y, 0.0);
           }
       }

   A trap that springs with nothing on screen reads as a bug in the log,
   and `every_trap_animates_when_it_springs` fails on one. If the
   animation only makes sense once the mechanic has run -- a teleport
   puffs on the tile the victim *left* -- name `no_flourish` and do it in
   the mechanic, the way the teleport trap and the trapdoor do.

3. **Add the row** to `TRAPS`, naming both:

       &TrapDef { name: "pit trap", glyph: '^', color: Color::Red,
                  weight: DEFAULT_SPAWN_WEIGHT, min_depth: 1, snare_turns: 0,
                  always_spent: false, apply: pit, flourish: dust },

   The name is what the player sees once the trap is known -- there is no
   separate label function to update. `weight` is how often the dungeon
   lays this one against the others it could lay; `min_depth` is the
   first floor it appears on. Every existing trap is
   `DEFAULT_SPAWN_WEIGHT` and `1`, which is to say equally likely from
   the start. `always_spent` uses the trap up every time it goes off, the
   way the bear trap's jaws stay shut on the victim; any other trap breaks
   `TRAP_BREAK_CHANCE` of the time. Both handler fields are required, so a
   row without them does not build. That is deliberate: a trap that does
   nothing is not a trap.

Then:

    cargo build
    cargo test --test it content::
    NIHILURK_SPAWN="pit trap" cargo run -p nihilurk

The pit trap is this page's example, so `a_traps_label_is_its_row` (`models/tests/it/content.rs`) asserts the game has none: copied as it stands, the row fails that test. Give yours its own name.


Writing the mechanic
--------------------

Look at what the existing traps do, and reuse the pieces:

    trapdoor_effect       moves the victim to the next floor down
    snare_victim(...)     Holds the victim for `TrapDef.snare_turns` turns
                          (bear trap pins the feet, gas takes the turn)
    teleport_effect       relocates the victim on this floor
    arrow_effect          damage, and drops a real arrow on a miss
    dart_effect           damage, plus a permanent bite of strength

If your trap snares, put the duration in the row's `snare_turns` and let `snare_victim` read it off `spring.row` -- no constant to add.

The arrow and dart also scale with depth. `trap_damage_tier(depth)` steps up at the depths in `TRAP_DAMAGE_TIER_LAST_DEPTH`, and it adds to the arrow's roll and the dart's drain. The bands and every per-tier amount are in `constants::traps` (`TRAP_DAMAGE_TIER_LAST_DEPTH` and friends). This band is the trap's own -- coarser than the floor-crowding `map::difficulty_tier`, on purpose.

Your handler receives a `Spring`:

    row         the trap's own row
    victim      whoever the trap got -- may be a monster, not the player
    is_player   whether it was
    seen        whether the player can see this happen
    at          where the trap is (or was), if you need it

> **A trap fires for monsters too.** `victim` is whoever moved onto the
> tile. Write the log line through the `seen` / `is_player` pair the way
> the existing handlers do, or the player will read second-person
> messages about a bat.

> **A trap fires for whoever is *near* it too.** Shoot a trap, or catch
> one in a wand's blast, and `detonate_trap` bursts it over the
> `TRICK_SHOT_RADIUS` around its tile: your handler then runs once per creature caught, none
> of whom is standing on the trap. So read `victim`'s own `Position` if
> you need where they are, take `at` as where the trap *was*, and do not
> assume the two are the same tile. The burst's own damage
> (`TRICK_SHOT_*` in `constants::traps`) is dealt before your handler
> runs, and the trap entity is already despawned by then -- a handler
> must never despawn it itself. `spring_trap` handles the stepped-on
> case, `always_spent` included. Bursts also chain, so your handler can
> run while another trap's burst is still resolving: re-read anything
> off the world that a burst could have changed, and never hold an
> `Entity` across a call that can explode.

> **Damage traps ignore the armour die but not the armour bonus.** If your
> trap deals damage, subtract `total_armor_plus(world, victim)` and
> nothing else -- see `arrow_effect`. Rolling a full opposed defence would
> make armour far better against traps than the design intends.

If your trap costs the victim turns, hold them: `conditions::snare(world, victim, Grant::of::<Asleep>(), turns)`, or `Pinned`, `Rooted` or `Clamped` (`effects::HOLDS` lists them). Each is an ordinary effect held for `Lifetime::Turns`, aged down once per turn by `effects::tick_effects` and lifted when its clock runs out. `Asleep` forfeits the victim's turn outright (`player_incapacitated`, and the AI skips the monster); `Pinned` only blocks movement -- a swing still lands, a step is a `bear_trap_thrash`; `Rooted` is `Pinned` without the teeth; `Clamped` is a creature's jaws, `clamped_thrash` in place of the bear trap's. You do not have to implement any of that; you pick which hold and for how long.

Each runs on its own clock, so a victim can be asleep *and* pinned, and comes out of each when its own turns are up.


Reveal styles
-------------

Every trap rolls one of three discovery styles at spawn, with equal odds, and you do not choose it per row:

    Sight       shows itself as soon as its tile enters the viewshed
    Adjacent    stays hidden until the player is standing next to it
    Triggered   invisible until it goes off

nihilurk has no "search" action, so these three are the only ways a trap ever comes to light before it bites. If you want a trap that is always visible or never visible, that is a change to `TrapBundle::random`, not to a row.


> **Never rename a trap row.** A saved trap stores its row's `name`, and
> a load that names no row is refused.


What you never have to do
-------------------------

  * Give the trap a label. `TrapDef::display_name` reads the row.
  * Add it to a list of trap kinds. `TrapDef::pick` walks the table.
  * Teach the save file about it. The save stores the row's name.
  * Place it. Floor generation decides how many traps and where; the table decides which.


Appendix: quick check
---------------------

1. Write the mechanic in `models/src/traps.rs`; log through `seen` and `is_player`, and subtract `total_armor_plus` only.
2. Write the flourish, or name `no_flourish` and do it in the mechanic.
3. Add the `TrapDef` row to `TRAPS` naming both: `weight`, `min_depth`, `snare_turns` if it holds, `always_spent` if it bites once.
4. Run `cargo build`, then `cargo test --test it content::`.
5. Run `NIHILURK_SPAWN="<name>" cargo run -p nihilurk`.


See also
--------

  tune-rarity-and-depth.md         make a trap rare, or deep-only
  ../reference/spawn-api.md        TrapDef::pick, TrapBundle::from_def
