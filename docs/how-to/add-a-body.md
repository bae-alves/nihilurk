How to add a new playable body
===============================

    Audience       Engine developer.
    Prerequisites  ../explanation/ecs-in-nihilurk.md, ../how-to/work-with-the-ecs.md.
                   ../tutorial/add-your-first-body.md is the long version,
                   as a lesson, if you want to feel the save-marker bug first.
    Result         A third hand-written `Body::Variant`, selectable with
                   `-b <name>`, that survives a save/load round trip.

Not a content-table recipe. `-am <species>` already lets anyone *wear* a
bestiary row (`add-a-monster.md` is that whole story) — this page is for the
other kind of body, one written by hand the way the lurk was: its own dice,
its own kit, its own rules about gear. Reach for this only when a species
from `-am` genuinely won't do.


The table
---------

    models/src/body.rs      ->  enum Body

Three variants today: `Nihil` (the default), `Lurk`, `Monster(&'static
MonsterDef)`. One enum, not a flag per body, because a player is exactly one
of them and the argument parser has nothing to cross-check — see the doc
comment at the top of `body.rs` before you touch it.


The recipe
----------

Follow `wear_lurk` line for line; a fourth body is a sibling of it, not a
new shape.

1. **Add the variant.**

       pub enum Body {
           Nihil,
           Lurk,
           Wraithkin,                       // new
           Monster(&'static MonsterDef),
       }

   `Body::name()` needs an arm too — it is what `-b` matches against and
   what an error about the body prints.

2. **Write `wear_wraithkin`, next to `wear_lurk`.** It inserts the starting
   `Renderable`, `Fighter`, `Magic` and `Speed` the same way, and grants
   whatever innate effects the body is born with through `grant_all` — a
   `const &[Grant]` slice, the same shape `LURK_GRANTS` uses, so it can be
   handed to `grant_all` unchanged. Pull every number out to its own
   `constants::wraithkin` module (see `../reference/constants.md` below) rather than
   writing them inline — that is the one line `wear_lurk` itself would fail
   review on if it didn't.

3. **Wire it into `wear`:**

       Body::Wraithkin => wear_wraithkin(world, player),

4. **Give it a marker that survives a save.** Neither the enum nor a field
   costs `saveload::EntitySave` anything — see the module doc comment on
   *why*: the save format isn't versioned, so a field it never had is a
   field an in-progress run can't come back from. A monster body is read
   back from `Name` (the species name doubles as the marker); the lurk
   is read back from its own `Lurk` effect marker, which rides the effect
   ledger like anything else a creature was born with. Your body needs the
   same: an effect (or existing marker) that (a) is already saved, and (b)
   nothing else in the game would attach to the player by accident.

5. **Teach `innate_tempo`** if the body doesn't return to `SpeedKind::Normal`
   after a staircase lifts whatever the floor lent it (the lurk always
   returns to `Quick`).

6. **Teach `equip_refusal`** if the body can't wear everything nihil can.
   The lurk refuses every slot but `Slot::Finger`; a monster body refuses
   everything unless its bestiary row carries `ItemUser`. Say *why* in the
   refusal string — it's what the player reads when they try.

7. **Wire the flag.** `engine/src/main.rs` matches `("-b", "nihil")` /
   `("-b", "lurk")` literally; add `("-b", "wraithkin") => body =
   Some((models::Body::Wraithkin, "-b"))`. `-b` and `-am` already refuse
   being passed together and refuse an unrecognised name — nothing else to
   touch there.

8. **`brings_a_pack`.** Only `Nihil` returns `true`. Leave it that way unless
   the new body is meant to start holding a mace, which none of the
   hand-written ones are — the whole point is a body with nothing to buy.


Checks that will catch you
---------------------------

`cargo test --test body` and `--test lurk` are the pattern to copy for a new
`--test wraithkin`: build a `World` with `StartingBody(Body::Wraithkin)`,
call `initialize_world`, then assert the glyph, the dice, the grants, and
that a save/load round trip comes back as the same body. Nothing else in the
suite exercises a body that doesn't exist yet.

`cargo test --test content` won't see this at all — a hand-written body is
not a table row, so none of its checks apply. That absence is the tell that
you're outside the tables and should be moving carefully, not the sign that
you can skip testing.


Ship it with the docs that describe it
---------------------------------------

Every one of these said something specific about the lurk when it landed,
and will say something wrong about your body if you don't touch it:

  * `../reference/cli-and-env.md` — the `-b <body>` section spells out
    each body's dice, kit and quirks for the player.
  * `../reference/constants.md` — a row in the modules table for the new
    `constants::<name>` module.
  * `../reference/components.md` — the "what the player is" section, if
    the new marker or resource is worth a reader reaching for.
  * `MANUAL.md` / `doc/nihilurk.6` — the shipped manual page.


See also
--------

  ../tutorial/add-your-first-body.md  the long version, as a lesson
  ../reference/cli-and-env.md      the player-facing story, `-b` and `-am` both
  add-a-monster.md                 the other way to play as something else
  add-an-effect.md                 writing the `Grant`s a body is born with
  ../reference/components.md       `Body`, `MonsterBody`, `StartingBody`
  ../explanation/combat-and-balance.md   picking dice that don't need gear to matter
