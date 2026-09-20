# Journal


## Petrification split out of sleep (2026-09-19)

A medusa's gaze used to *be* `Asleep`, so coming out of stone was reported with
the sleeper's line ("You shake off the drowsiness"). One effect serving two
flavours has exactly one place to put its sentence, so the flavour had to
become its own row: `Petrified`.

What it taught:

* An effect's `ends` line is the tell for a shared mechanic. Two things that
  end differently are two effects, however identical the mechanic looks the
  day it is written.
* `Petrified` is the first hold that is *not* in `HOLDS`. The array is "things
  holding a creature in a place", which is why both teleports let go of all of
  it; stone is the victim's own body and travels with them. Worth keeping that
  distinction explicit, because "add it to HOLDS" is the obvious wrong move.
* The damage cap had to go in two places — `combat::clamp_swing` (melee applies
  its own HP) and `helpers::apply_hit` (everything else) — so the rule and its
  sentence live together in `effects::stone_chip` and both paths ask it. Two
  copies would have been two answers to "can a petrified player die".
* `apply_hit`'s `announce` is the caller's sentence about a hit that, against
  stone, did not happen. The chip line replaces it. Callers that log their own
  damage number *before* calling (the arrow and dart traps) would still
  overstate — unreachable today, since a petrified player cannot step on a
  trap, but it is the shape of a future bug.


## The spell docs after the move→spell rename (2026-09-20)

The rename was a clean sed, which is exactly why the spell pages went stale
without breaking anything: every word was right and several facts were not.

What the audit turned up, and what it teaches:

* **A centralisation the docs never followed.** `MagicWard` is now checked once,
  in `helpers::apply_hit` (on `Hit::magical`), plus `abilities::fire_on_hit` for
  the rider on a blow. The how-to still told authors to check it per-victim and
  call `ward_block` — a function that does not exist. When a check moves into a
  shared function, the page that told people to write it by hand is the one
  thing that will keep them writing it by hand.
* **Markers moved into the `EFFECTS` ledger; the save advice didn't.** The docs
  said `MagicWard`/`Bided` are `bool` fields in `EntitySave`. They are rows in
  the registry, lent with a `Lifetime`, saved by id. Advice about persistence
  ages fastest, because it is the part nobody re-reads until a reload loses
  something.
* **A sed carried a balance change with it.** `power_mult` went 2→3 in the same
  uncommitted batch, silently making a staff triple damage while every comment,
  page and the `equip` test still said "doubles". A rename diff is where a
  number change hides best — read it for the lines that are *not* the rename.
* **`crate::ai` vs `crate::abilities`.** The dragon's fireball moved into an
  `ABILITIES` row at `Moment::InsteadOfAttacking` (its own commit said so); four
  doc sites still pointed at `ai`. `docs_style.sh` checks that source *paths*
  exist — it cannot check that a named *module* is still the right one.

Fixed at the root afterwards: the staff's two multipliers were bare literals in
two functions (`spell_cost`, `spell_system`) with their *meaning* written out in
six prose sites and a third literal asserted in `equip.rs`. They are now
`constants::spells::TURBO_MAGIC_COST_MULT` / `TURBO_MAGIC_POWER_MULT`, which is
what `constants.rs` says it is for — "every tuning knob in the model, in one
place". `docs/reference/constants.md` had no `spells` row at all, so the whole
module (thirteen knobs, since before the rename) was invisible to anyone
rebalancing from the docs.


## Second sight stopped being omniscience (2026-09-20)

`reveal_traps` treated `SeesInvisible` as a short-circuit *before* the viewshed
check, so a ring of perception (or a potion of see invisible) revealed and
logged every hidden trap on the floor, including ones in rooms the player had
never entered. Every other perception path in `visibility.rs` was already
gated: `hide_and_announce` computes `perceptible = in_view && (…)`. Traps were
the one place the `&& in_view` was missing, and the doc comment had
rationalised it ("reveals every trap on the floor at once") — which is how it
survived a test that asserted the bug by name.

The tell: a flag that widens *what* you perceive should never also widen
*where*. `perception` answers "can I see through invisibility", not "how far
can I see"; the moment it sits on the same line as `continue`, it is answering
both.

The test that caught it had to stop hardcoding a "far away" tile — `(2, 2)` was
inside the starting room on that seed. It now asks the player's own
`visible_tiles` for a tile genuinely out of view.


## Worn gear was still lying on the floor (2026-09-20)

Adding the `"(w. a short bow)"` tag to sighting and `look` lines turned up the
reason a hobgoblin's armour was being announced twice: `equip_silently` never
took the item's `Position` off. Monster gear is spawned *as loot on the tile*
and then put on, so every worn piece stayed a floor item — drawn under its
owner, spotted on its own line, and pickable off the ground while the monster
still got its bonus from it.

What it teaches:

* `drop_equipment` inserting a `Position` when gear comes off is the tell: if
  worn gear kept one, that insert would be redundant. The invariant ("worn is
  carried, carried has no `Position`") was already written down in
  `levels.rs::tear_down_the_floor` — just never enforced at the one door onto
  it.
* A test can be the thing that hides the bug. `saveload::round_trip` swept the
  floor clean by despawning everything with a `Position`, which silently also
  swept up worn gear; the moment worn gear lost its `Position`, the sweep
  missed it and the save grew an extra suit of armour. A cleanup filter that
  names a component is an assumption about that component.
* Articles are per-slot, not per-item: "a short bow" but "ring mail". `Slot::Body`
  is the whole rule, which is why `worn_phrase` takes a slot and not a name.
