How to add a new playable body
===============================

    Audience       Engine developer.
    Prerequisites  ../explanation/ecs-in-nihilurk.md, ../how-to/work-with-the-ecs.md.
                   ../tutorial/add-your-first-body.md is the long version,
                   as a lesson, if you want to feel the save-marker bug first.
    Result         A third hand-written `BodyDef` row, selectable with
                   `-b <name>`, that survives a save/load round trip.

Not a content-table recipe. `-am <species>` already lets anyone *wear* a
bestiary row (`add-a-monster.md` is that whole story) — this page is for the
other kind of body, one written by hand the way the lurk was: its own dice,
its own kit, its own rules about gear. Reach for this only when a species
from `-am` genuinely won't do.


The table
---------

    models/src/body.rs      ->  BODIES

Two rows today: nihil (the default) and lurk. A player is `Body::Row`
of one of them, or `Body::Monster(&'static MonsterDef)` for `-am`. One enum,
not a flag per body, because a player is exactly one of them and the
argument parser has nothing to cross-check — see the doc comment at the top
of `body.rs` before you touch it.


The recipe
----------

Follow `wear_lurk` line for line; a new body is a sibling of it, not a
new shape.

1. **Add the row** to `BODIES`:

       &BodyDef { name: "wraithkin", brings_a_pack: false, wear: wear_wraithkin },

   `name` is what `-b` matches, and a new player named it becomes it
   (`nihilurk wraithkin`). Only nihil brings a pack; leave
   `brings_a_pack` false unless the new body is meant to start holding a
   mace, which none of the hand-written ones are — the whole point is a
   body with nothing to buy.

2. **Write `wear_wraithkin`, next to `wear_lurk`.** It inserts the starting
   `Renderable`, `Fighter`, `Magic` and `Speed` the same way, and grants
   whatever innate effects the body is born with through `grant_all` — a
   `const &[Grant]` slice, the same shape `LURK_GRANTS` uses, so it can be
   handed to `grant_all` unchanged. Pull every number out to its own
   `constants::wraithkin` module (see `../reference/constants.md` below) rather than
   writing them inline — that is the one line `wear_lurk` itself would fail
   review on if it didn't.

3. **Give it a marker that survives a save.** A monster body is the one
   body the save writes down (`SaveGame::monster_body`, the species id);
   the lurk is read back from its own `Lurk` effect marker, which rides the
   effect ledger like anything else a creature was born with. Your body
   needs the same: an effect (or existing marker) that (a) is already
   saved, and (b) nothing else in the game would attach to the player by
   accident. That marker is how the game knows the body after a load:
   `innate_tempo` and `equip_refusal` read `Lurk` the same way, and so
   does `conduct::kept_mask`. A new saved field changes the save format: see the
   `EntitySave` note in `add-an-item-category.md`.

4. **Teach `innate_tempo`** if the body doesn't return to `SpeedKind::Normal`
   after a staircase lifts whatever the floor lent it (the lurk always
   returns to `Quick`).

5. **Teach `equip_refusal`** if the body can't wear everything nihil can.
   The lurk refuses every slot but `Slot::Finger`; a monster body refuses
   everything unless its bestiary row carries `ItemUser`. Say *why* in the
   refusal string — it's what the player reads when they try.

6. **Name it in the refusal.** `-b` looks the name up in `BODIES`, so the
   flag needs nothing. An unknown name prints `no_such_body` (`strings/src/en.rs`;
   the other languages re-export it), which lists the bodies by hand: add yours there.


Checks that will catch you
---------------------------

`models/tests/it/body.rs` and `lurk.rs` are the pattern to copy for a new
`wraithkin.rs`, declared as `mod wraithkin;` in `models/tests/it/main.rs`
and run with `cargo test --test it wraithkin::`. Build a `World` with
`StartingBody(Body::Row(&WRAITHKIN))`,
call `initialize_world`, then assert the glyph, the dice, the grants, and
that a save/load round trip comes back as the same body. Nothing else in the
suite exercises a body that doesn't exist yet.

`cargo test --test it content::` won't see this at all — `BODIES` is not one of
the content tables it walks, so none of its checks apply. That absence is the tell that
you're outside the item and monster tables and should be moving carefully, not the sign that
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


Appendix: quick check
---------------------

1. Add the `BodyDef` row in `models/src/body.rs`, and list it in `BODIES`.
2. Write `wear_<name>` next to `wear_lurk`, with every number in a new `constants::<name>` module.
3. Give it a save marker: an effect already in `EFFECTS` that nothing else puts on the player.
4. Teach `innate_tempo` if the body does not return to `SpeedKind::Normal`.
5. Teach `equip_refusal` if the body cannot wear everything, and say why in the string.
6. Add the name to `no_such_body` in `strings/src/en.rs`; the other languages re-export it.
7. Leave `brings_a_pack` false.
8. Copy `models/tests/it/body.rs` and `lurk.rs` into a new test file named for the body: glyph, dice, grants, save round trip.
9. Run that test with `cargo test --test it <name>::`, then `cargo test`.
10. Fix the docs: `update-the-docs.md`, "A playable body".


See also
--------

  ../tutorial/add-your-first-body.md  the long version, as a lesson
  ../reference/cli-and-env.md      the player-facing story, `-b` and `-am` both
  add-a-monster.md                 the other way to play as something else
  add-an-effect.md                 writing the `Grant`s a body is born with
  ../reference/components.md       `Body`, `MonsterBody`, `StartingBody`
  ../explanation/combat-and-balance.md   picking dice that don't need gear to matter
