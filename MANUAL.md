Roog — how to play
===================

A classic roguelike: fight down thirteen floors, take the Element of Yoord
off the bottom, carry it back up thirteen floors to the surface. One
character, one life, one save file that gets deleted the moment the run
ends. Life is unfair. This document tells you which buttons do what and
which numbers decide whether you live.

For what the game is *trying to be*, read `gdd.md`. For how to add content
to it, read `docs/`. This page is neither — it's the manual you'd get in
the box, if roog shipped in a box.


Starting a run
--------------

    cargo run -p engine                  # play as "Roog"
    cargo run -p engine -- YourName       # play under a name
    cargo run -p engine -- -s 1234        # a specific seed, reproducible
    cargo run -p engine -- roog           # loads roog.sav, if it exists

A bare word on the command line is your name, unless it happens to match a
save file already sitting there (with or without `.sav`), in which case
that run is loaded instead.

There is exactly one save. It's written when you quit mid-run and deleted
the instant a run actually ends, win or lose — it exists so you can stop
playing, not so you can undo a death. If you've already won once, the save
is kept as "clear data"; loading it again asks whether you're sure you want
to spend it, since starting over hands the Element back to the Dungeon
Lord.


Reading the screen
-------------------

Row 0 is the status line:

    Roog · HP 9/12 · Ma 4/4 · Pow. 1d4+1 · Arm. 1d3+1 · DEPTH 3 · SCORE 140

`Pow.` and `Arm.` are your *current* attack and defence dice, already
folding in every weapon, armour and ring you have on — what you see there
is exactly what combat rolls against. `Thr.` appears next to them once
something is boosting your throws. The last field is normally your score
(coins carried), but if something noteworthy is going on it's replaced by a
badge instead:

| Badge                 | Means |
|-----------------------|-------|
| `FAST` / `SLOW`       | Hasted or slowed (a potion, a wand, a trap) |
| `CONF`                | Confused — steps go random half the time |
| `HELD`                | Caught in a bear trap |
| `ASLEEP`              | Down in sleeping gas |
| `EXPLORING` / `TRAVELING` | Auto-explore or `O`-travel is walking for you |
| `ASCENDING`           | You're carrying the Element of Yoord |
| `TRAVEL?`             | The `O` travel cursor is up |

Below the map, the last few log lines. A `--MORE--` prompt eats every key
except Space/Enter until you clear it — nothing else is listening while
it's up.

The map itself, glyph by glyph:

| Glyph | Meaning |
|-------|---------|
| `.`   | Room floor |
| `▒`   | Corridor |
| `#`,`\|`, etc.   | Wall |
| `+`   | Door |
| `>` / `<` | Stairs down / up |
| `@`   | You |
| A letter (`g`, `T`, `D`, ...) | A monster — see the bestiary below |
| `%`   | A corpse |
| `≈`   | Smoke — harmless, fades on its own |
| `^`   | A discovered trap |
| `$`   | Coins |
| `!` `?` `/` `=` | Potion, scroll, wand, ring |
| `)` `]` `}` | Weapon/ammo, armour, launcher (bow/crossbow) |
| `"`   | The Element of Yoord |

A tile you've seen but can't currently see is drawn dim grey — remembered,
not live. Blood stains the floor wherever fighting happened and stays for
the rest of the run (`-nb` turns this off if you'd rather not see it).

The map itself lurches when something lands hard: a short kick every time
something dies where you can see it, a heavier one for your own excellent
hits and for a blast going off in sight, a long one the moment a wound
drops you into "You are badly wounded!" territory, and the biggest of the
lot when you die. Only the map moves — the status line and the message log stay
put, so the line telling you what just happened is readable while it is
happening. It never costs you a keystroke either: press anything and the
map settles at once and your key is taken as normal. `-nshake` turns it
off entirely.


Controls
--------

Movement is eight-directional. Use whichever scheme you already know —
they're all live at once:

| Direction | Arrows | vi (hjkl) | Numpad |
|-----------|--------|-----------|--------|
| N / S / W / E | ↑ ↓ ← → | k j h l | 8 2 4 6 |
| NW / NE / SW / SE | — | y u b n | 7 9 1 3 |

**WASD no longer moves you** — those letters are commands now (and so is
`q`, so quit with `Esc`).

Walking into a monster attacks it instead — there's no separate attack key.
Walking into a wall diagonally when it would "cut the corner" is refused;
you have to go around. Attacking walls is likewise not possible.

| Key | Does |
|-----|------|
| Shift + a direction | **Run** that way (or toward the nearest stairs/door/item roughly that way) until something interesting happens |
| `o` | **Auto-explore** the floor — beelines for any spotted item before resuming |
| `A` | Turn that beelining **off or on** (`Pick-up on auto-explore OFF/ON.`) — it also switches itself off while your pack is full |
| `O` | Drop a **travel cursor** — steer it with the direction keys, Enter to walk there |
| `Tab` | **Auto-fight** whatever's closest (refuses below 25% HP or while confused); with a bow/crossbow drawn, fires at it instead of closing in |
| `f` | **Fire** the wielded launcher's first matching arrow/quarrel — opens the aiming reticle, same as Throw |
| `>` or `.` | Go down / walk to the down stairs |
| `<` or `,` | Go up / walk to the up stairs |
| `x` or `X` | Close whatever's open — the universal escape hatch, never spends a turn |
| `Q` or `X` | With nothing open: quit — asks **Really quit?** first, `y` to confirm, `n` to think better of it |
| Ctrl+C | Quit at once, no question asked |

`X` is only ever a quit key with nothing on screen to close; with a menu,
a reticle or the prompt itself up it just closes that, same as `x`. And
`Esc` does **not** quit at all — it backs out of menus, and that's all.
Quitting saves your run (there's exactly one save; see above).

And ten keys that open the pack, each showing only what it can act on:

| Key | Menu | Shows |
|-----|------|-------|
| `i` | your **pack** | everything, and asks Use / Throw / Drop afterwards |
| `a` | **use** what? | everything — the one-key version of `i` → Use |
| `t` | **throw** what? | everything (the aiming reticle opens next) |
| `d` | **drop** what? | everything |
| `e` | **equip** what? | anything wearable or wieldable |
| `q` | **quaff** what? | potions |
| `r` | **read** what? | scrolls |
| `w` | **wield** what? | weapons, bows and crossbows |
| `W` | **wear** what? | armour |
| `P` | **put on** what? | rings |

Pick a row with its letter (or the direction keys and Enter) and it
happens at once — no second menu. An item keeps the same letter in every
one of these menus, so the potion that's `c` in your pack is `c` in the
quaff menu too. If there's nothing to show, the game just says so
("You have nothing to read.") and you keep your turn.

Any run/auto-explore/travel stops dead the instant a monster comes into
view, so it's safe to spam. **You cannot pass your turn** — if you press a
key that isn't a real action, nothing happens and no turn is spent, but
there's no "wait here" or "search" command either.


The pack
--------

`i` opens it: arrow keys or the item's own letter to select, Enter for the
action menu — **Use**, **Throw**, **Drop**. The other nine pack keys above
skip that menu and do one of those three directly, so `i` is for when you
want to look at what you're carrying first.

- **Use** on a potion or scroll drinks/reads it immediately. On a wand, a
  bow, or anything else that needs a target, it opens an aiming reticle
  first — move it with the direction keys, Enter to fire, `Esc`/`x` to back
  out. Equipping/unequipping gear (wielding a weapon, wearing armour,
  putting on a ring) also goes through Use.
- **Throw** always opens the reticle. A dagger or spear is balanced for
  flight and pierces everything on its line; a mace is an improvised lump
  that a monster with hands can catch and use back on you; arrows and
  quarrels come out of the stack one at a time and hit twice as hard if
  you're holding the matching bow/crossbow.
- **Drop** puts it back on the floor. Refused for anything cursed and worn
  — it won't come off (see Curses, below) — and refused for the Element of
  Yoord, which doesn't leave your hands until the game is over.

Ammunition and coins stack (up to 13 per slot); everything else takes its
own slot. You have 9 slots, capped there on purpose so every slot's letter
stays out of the way of the ones the game reserves for scrolling the list.


Combat
------

There's no to-hit roll and nothing ever misses outright. Every fight is two
independent rolls, subtracted:

    damage = (1d[Power] + PowerBonus) − (1d[Armor] + ArmorBonus)

If that comes out zero or negative, nothing happens. `Pow.`/`Arm.` on your
status line are already the totals — weapon, armour, and every ring you
have on folded together into the same four numbers. The game never
remembers *what kind* of item lent you a bonus; a longsword, a suit of
plate, and a ring of protection are indistinguishable to the combat code.

Two rules exist only for you, never for monsters:

- **Excellent hit** — 15% of your swings roll three attack dice instead of
  one (summed, before armour comes off), a shot at cracking good armour
  that a monster's plain 1d[Power] never gets.
- **Chip damage** — even your worst possible swing still takes 1 HP off if
  it would otherwise have done nothing, but a hit that only exists because
  of this floor can never be the killing blow; it leaves a foe on 1 HP.
  Monsters get no such floor — if a monster's roll can't beat your armour,
  it does nothing at all, which is the entire reason armour is worth
  wearing.

HP totals are small (you start at 12) — a couple of bad rolls in a row is a
real threat the whole game, there's no leveling up to outgrow it.

Kill something wearing gear and each piece it had on gets its own
coin-flip: heads, it lands on the corpse's tile and gets called out in the
log so you know to go back for it; tails, it's destroyed with its owner.


Status effects
---------------

- **Confused** — every step has a coin-flip chance of stumbling in a random
  direction instead (still costs the turn, even into a wall). Running,
  auto-explore and travel all refuse to start while confused.
- **Fast / Slow** — you (or a monster) act twice as often, or half as
  often. Wands of haste/slow monster shift a creature one notch on this
  scale permanently; a hasted or slowed *player* loses it on the next
  staircase.
- **Held** (bear trap) — pins your feet, not your fists: you can still
  swing at whatever's adjacent, but trying to step anywhere just thrashes
  against the trap, wasting the turn and tearing a point of HP.
- **Asleep** (sleeping gas) — the turn is forfeited outright, no key read,
  until it wears off.

Everything else in the original list of Rogue punishments — blindness,
paralysis, poison, being turned to stone — comes from potions that exist
and are drinkable but, in this build, don't yet do anything beyond telling
you what they are. See "What doesn't do anything yet" below.


Exploring the dungeon
-----------------------

You always see the 3×3 around you. Standing in a **lit room** floods the
whole room into view, walls and doorways included; everywhere else is
corridor sight — just what's directly ahead. About 15% of rooms past the
first spawn dark and behave like a corridor until a wand of light goes off
in them. Monsters vanish from the map the moment they leave your sight;
the floor layout itself, once seen, stays remembered (dim grey) for the
rest of your time on that floor. Fog of war is **not** carried between
visits — walk back up through a floor you already cleared and it's blank
again, even though the walls are identical.

Traps come in six kinds and reveal themselves in one of three ways, picked
at random per trap, so a room is never a solved puzzle:

- Some show the moment the tile enters your view.
- Some stay hidden until you're standing right next to them.
- Some are invisible until they actually go off.

The arrow trap's damage ignores your armour die entirely (not your armour
bonus) and gets nastier the deeper you are, and the dart trap does the
same *plus* permanently drains a point of Power — a ring of strength is
the only protection. A trapdoor, a bear trap, sleeping gas, and a teleport
fill out the rest.

**The Dungeon Lord's patience.** You get 260 turns on any one floor. Run
past that and a portal opens under you and drops you a level deeper whether
you were ready or not (or, once you're carrying the Element, a level
*shallower* — the impatience runs the other way on the climb out). This is
most of the reason the automation exists: `o` and `Tab` chew through a
floor fast enough that patience is rarely the thing that gets you.

A staircase is also a rest: arriving on a new floor heals half your missing
HP and refills your magic pool completely. A trapdoor plunge skips both —
it's a fall, not a rest.

Seeds fix the *maps*, never the contents. The same seed always carves the
same floor 7, every time, forever — but what's standing in it, what's lying
around, and what's trapped is re-rolled fresh every time you enter,
including walking back up through a floor you already cleared.


Magic
-----

A second resource pool, `Ma X/Y` on the status line, 4 points at the start
of a run. It does not regenerate on its own — only a staircase refills it.
(At the moment nothing in the game actually spends it yet; treat the number
as a preview of a mechanic still being wired in.)


Items
-----

Nothing arrives identified. Every potion, scroll, wand and ring has a
cosmetic disguise — a colour, a garbled title — shuffled fresh each seed,
and drinking or reading one identifies *every* item of that same true type
you'll ever find for the rest of the run, not just the one in your hand.
Weapons, armour and launchers hide something too: whether they're plus or
cursed, and by how much, until you actually wear them or read a scroll of
identify.

**Curses.** 65% of the gear the dungeon drops is cursed — it can still roll
a good number, the gamble isn't the stat, it's that the moment you equip it
it welds on and nothing but a scroll of remove curse gets it back off
(which destroys the item rather than freeing it clean). 25% of drops are
plain, 10% are exceptional (+1 to +3, and safe to take off any time).

You start a run already equipped: +1 ring mail worn, +1 mace wielded, a +1
short bow and 13 arrows in the pack, and one potion of healing you already
recognise.

### Weapons

| Weapon | Die | Notes |
|--------|-----|-------|
| Dagger | d4 | Thrown: pierces the whole line |
| Spear | d6 | Thrown: rolls d8, pierces the whole line |
| Mace | d6 | |
| Long sword | d8 | |
| Two-handed sword | d10 | |

### Armour

| Armour | Die |
|--------|-----|
| Leather | 2 |
| Ring mail | 3 |
| Studded leather | 4 |
| Scale mail | 5 |
| Chain mail | 6 |
| Splint mail | 7 |
| Banded mail | 8 |
| Plate mail | 9 |

### Ammunition and launchers

Arrows (d4) and quarrels (d6) stack up to 13 to a slot and are useless
without a matching bow or crossbow — with one, the die doubles. A bow or
crossbow itself has nothing to swing with; it occupies the hand a sword
would have, and clubbing someone with one is worth at most 1 damage
whatever its enchantment. One is a build; a second is dead weight.

With a bow or crossbow drawn, `f` skips the pack: it grabs the first
matching arrow/quarrel and opens the aiming reticle straight away, same as
`i` → item → Throw but one key. `Tab` changes meaning too — instead of
walking up to the nearest threat, it fires at it on the spot, refusing with
"You're out of ammo." or "No clear shot." rather than ever closing the
distance by hand.

### Wands

`3d3` damage on the attack wands, `2d6+1` charges rolled when a wand
enters the dungeon, range 6 or 8 depending on the wand. All fourteen do
something when zapped — nothing here is a dud. A few worth knowing about
in advance: light needs no target at all, floods the room or passage
you're standing in, and reveals any hidden trap in it; polymorph and the
haste/slow-monster wands change a creature rather than just hurting it;
and cancellation is the one to dread catching yourself in — it strips
every magic effect off whatever it hits, and on you that's a catastrophe:
every enchantment on your gear zeroes out, every unread scroll goes blank,
every potion turns to water.

A thrown wand dumps its whole remaining charge at once and bursts wider
and hotter than a zap does.

### Rings

Worn on a finger — you have two slots. Five of the twelve do something
right now: straightforward, permanent effects like extra armour, extra
power, and seeing what's normally invisible, plus one whose curse is a
genuine trap rather than a stat penalty — a flat chance per action of
aggravating the entire floor. The other seven are real, correctly
identified, and findable, but currently inert (see "What doesn't do
anything yet" below).

### Potions

| Potion | Effect |
|--------|--------|
| Healing | Heals you to full |
| Thirst quenching | Nothing — the "you found plain water" potion |

Confusion, extra healing, gain strength, haste self, monster detection,
magic detection, raise level, restore strength, see invisible, blindness,
paralysis, and poison all exist, are findable and drinkable, and currently
do nothing beyond telling you what they are the first time you try one.

### Scrolls

Nine scroll types do something on purpose (a tenth, blank paper, does
nothing — also on purpose). Remove curse frees every cursed item you're
wearing, but destroys it rather than handing it back clean; vorpalize
weapon brands your wielded weapon as the bane of one random species, and
reading it a second time destroys the weapon instead of stacking; and
magic mapping reveals the floor as an animated wipe instead of snapping
the whole layout into view at once. Identify, teleportation, aggravate
monsters, create monster, and scare monster round out the rest of the
working list.

Monster confusion, hold monster, sleep, enchant armor, enchant weapon, and
food detection exist and are readable, and currently do nothing beyond
telling you what they are.

### Coins

Gold (1000) and silver (100). They buy nothing — they *are* the score.


The bestiary
-------------

Twenty-six species, one letter each, sorted into three danger tiers:
fodder appears from floor 1, a middle tier joins from floor 5, and the
nastiest letters start showing up from floor 10 — early fodder never
stops appearing, it just starts arriving in worse company. Most letters
chase you on sight; a few flee instead, a few never move at all, and one
staggers around at random the same way you do while confused.

A handful are worth knowing about before you meet them: several species
will pick up whatever you throw at them and use it right back at you, a
few are undead, a couple are outright immune to a specific element, one is
permanently invisible, and exactly one is a legal target for any vorpal
weapon. The rest is for you to find out on the way down.

Claiming the Element of Yoord tears every one of those depth gates off its
hinges: for the whole climb back up, every floor draws from the entire
bestiary, no exceptions.


The Element of Yoord, and winning
-----------------------------------

Depth 13 has no down staircase. The Element sits where it would be.
Picking it up flips the rule for the rest of the run: down is dead, up is
the only way, and the 260-turn impatience now throws you
*upward* through a level instead of down if you dawdle. The very last
climb, out of depth 1, has to be walked on foot — a portal can never be the
thing that wins the game for you.

Walking out of depth 1 carrying the Element is the win condition.
Everything else is a loss, and most runs are a loss.


Dying (and everything else)
------------------------------

Death is permanent, there is no retry. A "You die..." panel, a
`--MORE--`, then a tombstone, what killed you, and your score.

Most runs end from attrition, not a single big monster: a string of fights
with no healing potion left, and something ordinary finishes what a
tougher fight started. The rest end by putting on an unidentified cursed
ring to find out what it does, wandering one room too far past the Dungeon
Lord's patience, or a trapdoor at the wrong moment.


What doesn't do anything yet
-------------------------------

This build is a work in progress and it's honest about it: every item
below is real, spawns in the dungeon, gets identified normally, and is
completely safe to pick up — it simply has no mechanical effect yet when
you use it. Nothing here is a bug you need to work around; it's just not
wired in yet.

  * All potions except **healing**.
  * Six scrolls: **monster confusion, hold monster, sleep, enchant armor,
    enchant weapon, food detection**.
  * Seven rings: **adornment, increase damage, regeneration, slow
    digestion, teleportation, stealth, maintain armor**.
  * The magic points pool itself — nothing currently spends `Ma`.

Everything else described in this manual — every wand, every working ring,
the scrolls not listed above, and healing — is live.


Useful command-line flags
----------------------------

The full list, with every environment variable, is
`docs/reference/cli-and-env.md`. The ones worth knowing as a player:

| Flag | Effect |
|------|--------|
| `-s <seed>` | Play a specific, reproducible seed |
| `-ns` | Don't write a save file at all |
| `-nb` | Turn off blood and the corpse-fling death animation |
| `-nshake` | Turn off the screen shake |
| `-c` | Centre the map on you instead of a fixed viewport |
| `-anim-rate <n>` | Speed up (`<1`) or slow down (`>1`) particle, magic-map and screen-shake animations |


Quick reference
------------------

    Move        arrows / hjkl / numpad, diagonals yubn or 7913
    Attack      walk into it
    Run         Shift + direction
    Auto-explore   o        A toggles picking things up
    Travel      O, steer, Enter
    Auto-fight  Tab
    Fire        f        (bow/crossbow drawn)
    Stairs      > down, < up (or . and ,)
    Pack        i        Use / Throw / Drop
    Use/throw/drop  a / t / d
    Quaff/read  q / r
    Equip       e        w wield, W wear, P put on
    Cancel      x or X   (never spends a turn)
    Quit        Q or X   with nothing open; asks first · Ctrl+C doesn't

    damage = (1d[Power] + bonus) - (1d[Armor] + bonus)
    15% of your swings roll 3 dice instead of 1
    your worst swing still takes 1 HP, but can't finish the kill
