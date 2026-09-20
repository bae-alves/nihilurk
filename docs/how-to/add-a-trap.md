How to add a trap
=================

    Audience       Content author.
    Prerequisites  Comfortable adding a row. See
                   `../tutorial/add-your-first-item.md`.
    Result         A new trap the dungeon lays on floors, that reveals
                   itself in the usual three ways, and that survives a
                   save.

Four edits, and the compiler enforces the last two.


The mechanics are one file:

    models/src/traps.rs

The components a trap is built from -- `Trap`, `TrapEffect`, `TrapReveal`, `EntityMoved` -- are nouns and live in `models/src/components.rs` with everything else the ECS is made of. The three holds a trap can put on a victim (`Asleep`, `Pinned`, `Rooted`) are ordinary effects and live in `models/src/effects.rs`. You add a `TrapEffect` *variant* there; the row and the mechanic stay here.


The recipe
----------

1. **Append a variant** to `TrapEffect` (in `components.rs`):

       pub enum TrapEffect {
           ...
           Dart,
           Pit,        // <- at the end; see the save-format warning
       }

2. **Add the row** to `TRAPS` (in `traps.rs`):

       TrapDef { effect: TrapEffect::Pit, name: "pit trap",
                 glyph: '^', color: Color::Red, weight: 10, min_depth: 1 },

   The name is what the player sees once the trap is known -- there is no
   separate label function to update. `weight` is how often the dungeon
   lays this one against the others it could lay; `min_depth` is the
   first floor it appears on. All six existing traps are `10` and `1`,
   which is to say equally likely from the start.

3. **Write the mechanic** as an arm of `apply_trap_effect`:

       TrapEffect::Pit => pit_effect(world, victim, is_player, seen),

   `apply_trap_effect`'s match has no catch-all, so the compiler will
   refuse to build until you write this arm. That is deliberate: a trap
   that does nothing is not a trap.

4. **Say what it looks like** as an arm of `trap_flourish`:

       TrapEffect::Pit => {
           if let Some(mut fx) = world.get_resource_mut::<Particles>() {
               fx.poof(p.x, p.y, 0.0);
           }
       }

   Same deal: no catch-all, so the compiler refuses to build until the
   new trap answers "and what does it look like?". A trap that springs
   with nothing on screen reads as a bug in the log. If the animation
   only makes sense once the mechanic has run -- a teleport puffs on the
   tile the victim *left* -- write an empty arm saying so and do it
   there, the way `Teleport` and `Trapdoor` do.

Then:

    cargo build
    cargo test --test content
    NIHILURK_SPAWN="pit trap" cargo run -p engine


Writing the mechanic
--------------------

Look at what the existing six do, and reuse the pieces:

    trapdoor_effect       moves the victim to the next floor down
    snare_victim(...)     Holds the victim for `TrapDef.snare_turns` turns
                          (bear trap pins the feet, gas takes the turn)
    teleport_effect       relocates the victim on this floor
    arrow_effect          damage, and drops a real arrow on a miss
    dart_effect           damage, plus a permanent bite of strength

If your trap snares, put the duration in the row's `snare_turns` and let `apply_trap_effect` pass it through -- no constant to add.

The arrow and dart also scale with depth. `trap_damage_tier(depth)` is 0/1/2, stepping at floors 4 and 8, and it adds to the arrow's roll and the dart's drain. The bands and every per-tier amount are in `constants::traps` (`TRAP_DAMAGE_TIER_LAST_DEPTH` and friends). This band is the trap's own -- coarser than the floor-crowding `map::difficulty_tier`, on purpose.

Your arm receives:

    world       the ECS world
    victim      whoever the trap got -- may be a monster, not the player
    is_player   whether it was
    seen        whether the player can see this happen
    trap_pos    where the trap is (or was), if you need it

> **A trap fires for monsters too.** `victim` is whoever moved onto the
> tile. Write the log line through the `seen` / `is_player` pair the way
> the existing arms do, or the player will read second-person messages
> about a bat.

> **A trap fires for whoever is *near* it too.** Shoot a trap, or catch
> one in a wand's blast, and `detonate_trap` bursts it over the 3x3
> around its tile: your arm then runs once per creature caught, none of
> whom is standing on the trap. So read `victim`'s own `Position` if you
> need where they are, take `trap_pos` as where the trap *was*, and do
> not assume the two are the same tile. The burst's own damage
> (`TRICK_SHOT_*` in `constants::traps`) is dealt before your arm runs,
> and the trap entity is already despawned by then -- an arm must never
> despawn it itself. `spring_trap` handles the stepped-on case; the bear
> trap's "bites once" despawn lives there, not in the arm.

> **Damage traps ignore the armour die but not the armour bonus.** If your
> trap deals damage, subtract `total_armor_plus(world, victim)` and
> nothing else -- see `arrow_effect`. Rolling a full opposed defence would
> make armour far better against traps than the design intends.

If your trap costs the victim turns, hold them: `conditions::snare(world, victim, Grant::of::<Asleep>(), turns)`, or `Pinned`, or `Rooted`. Each is an ordinary effect held for `Lifetime::Turns`, aged down once per turn by `effects::tick_effects` and lifted when its clock runs out. `Asleep` forfeits the victim's turn outright (`player_incapacitated`, and the AI skips the monster); `Pinned` only blocks movement -- a swing still lands, a step is a `bear_trap_thrash`; `Rooted` is `Pinned` without the teeth. You do not have to implement any of that; you pick which hold and for how long.

The three run on their own clocks, so a victim can be asleep *and* pinned, and comes out of each when its own turns are up.


Reveal styles
-------------

Every trap rolls one of three discovery styles at spawn, with equal odds, and you do not choose it per row:

    Sight       shows itself as soon as its tile enters the viewshed
    Adjacent    stays hidden until the player is standing next to it
    Triggered   invisible until it goes off

nihilurk has no "search" action, so these three are the only ways a trap ever comes to light before it bites. If you want a trap that is always visible or never visible, that is a change to `TrapBundle::random`, not to a row.


> **Append `TrapEffect` variants; never insert or reorder.** A saved trap
> stores its effect as a position in the enum. Inserting a variant in the
> middle turns every saved trapdoor into something else.


What you never have to do
-------------------------

  * Give the trap a label. `TrapEffect::label()` reads the row.
  * Add it to a list of trap kinds. `TrapDef::pick` walks the table.
  * Teach the save file about it, beyond the enum variant.
  * Place it. Floor generation decides how many traps and where; the table decides which.


See also
--------

  tune-rarity-and-depth.md         make a trap rare, or deep-only
  ../reference/content-tables.md   the TrapDef fields
  ../reference/spawn-api.md        TrapDef::pick, TrapBundle::from_def
