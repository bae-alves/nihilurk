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
