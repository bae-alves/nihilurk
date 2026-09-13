Roog — how to play
===================

A classic roguelike: fight down thirteen floors, take the Element of Yoord off the bottom, carry it back up thirteen floors to the surface. One character, one life, one save file that gets deleted the moment the run ends. Life is unfair. This document tells you which buttons do what and which numbers decide whether you live.

For what the game is *trying to be*, read `gdd.md`. For how to add content to it, read `docs/`. This page is neither — it's the manual you'd get in the box, if roog shipped in a box.


Starting a run
--------------

    cargo run -p engine                  # play as "Roog"
    cargo run -p engine -- YourName       # play under a name
    cargo run -p engine -- -s 1234        # a specific seed, reproducible
    cargo run -p engine -- roog           # loads roog.sav, if it exists

A bare word on the command line is your name, unless it happens to match a save file already sitting there (with or without `.sav`), in which case that run is loaded instead.

There is exactly one save. It's written when you quit mid-run and deleted the instant a run actually ends, win or lose — it exists so you can stop playing, not so you can undo a death. If you've already won once, the save is kept as "clear data"; loading it again asks whether you're sure you want to spend it, since starting over hands the Element back to the Dungeon Lord.

**Your gear comes off across a save.** Everything is still in your pack — nothing is lost, and cursed items are still cursed — but you reload with empty hands and bare skin, so re-wield and re-wear before you take a step. The floor you left is also scrubbed clean: blood, corpses and smoke are never written to the file.


Reading the screen
-------------------

Row 0 is the status line:

    Roog · HP 9/12 · Ma 4/4 · Pow. 1d4+1 · Arm. 1d3+1 · DEPTH 3 · SCORE 140

`Pow.` and `Arm.` are your *current* attack and defence dice, already folding in every weapon, armour and ring you have on — what you see there is exactly what combat rolls against. `Thr.` appears next to them once something is boosting your throws. The last field is normally your score, but if something noteworthy is going on it's replaced by a badge instead — and whenever you earn anything it flashes what you just earned, in a colour picked at random, before going back to being a number:

| Badge                 | Means |
|-----------------------|-------|
| `FAST` / `SLOW`       | Hasted or slowed (a potion, a wand, a trap) |
| `CONF`                | Confused — steps go random half the time |
| `BLND`                | Blind — you can only feel the squares you could touch |
| `PARL`                | Paralysed — slowed, and losing half the turns that leaves you |
| `GLOW`                | Your hands are charged — the next blow you land confuses what it hits |
| `HELD`                | Caught in a bear trap |
| `ASLEEP`              | Down in sleeping gas (yours, or a trap's) |
| `EXPLORING` / `TRAVELING` | Auto-explore or `O`-travel is walking for you |
| `ASCENDING`           | You're carrying the Element of Yoord |
| `TRAVEL?`             | The `O` travel cursor is up |

Below the map, the last few log lines. A `--MORE--` prompt eats every key except Space/Enter until you clear it — nothing else is listening while it's up.

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

A tile you've seen but can't currently see is drawn dim grey — remembered, not live. Blood stains the floor wherever fighting happened and stays for the rest of the run (`-nb` turns this off if you'd rather not see it).

The map itself lurches when something lands hard: a tick every time one of your own blows, shots or bolts gets through a monster's armour, a slightly longer kick every time something dies where you can see it, a heavier one for your excellent hits and for a blast going off in sight, a long one the moment a wound drops you into "You are badly wounded!" territory, and the longest of the lot when you die. Nothing you can't see ever shakes the map. Only the map moves — the status line and the message log stay put, so the line telling you what just happened is readable while it is happening. It never costs you a keystroke either: press anything and the map settles at once and your key is taken as normal. `-nshake` turns it off entirely.


Controls
--------

Movement is eight-directional. Use whichever scheme you already know — they're all live at once:

| Direction | Arrows | vi (hjkl) | Numpad |
|-----------|--------|-----------|--------|
| N / S / W / E | ↑ ↓ ← → | k j h l | 8 2 4 6 |
| NW / NE / SW / SE | — | y u b n | 7 9 1 3 |

**WASD no longer moves you** — those letters are commands now (and so is `q`, so quit with `Esc`).

Walking into a monster attacks it instead — there's no separate attack key. Walking into a wall diagonally when it would "cut the corner" is refused; you have to go around. Attacking walls is likewise not possible.

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

`X` is only ever a quit key with nothing on screen to close; with a menu, a reticle or the prompt itself up it just closes that, same as `x`. And `Esc` does **not** quit at all — it backs out of menus, and that's all. Quitting saves your run (there's exactly one save; see above).

And eleven keys that open the pack, each showing only what it can act on:

| Key | Menu | Shows |
|-----|------|-------|
| `i` | your **pack** | everything, and asks Use / Throw / Drop afterwards |
| `a` | **use** what? | everything — the one-key version of `i` → Use |
| `t` | **throw** what? | everything (the aiming reticle opens next) |
| `d` | **drop** what? | everything |
| `e` | **equip** what? | anything wearable or wieldable |
| `q` | **quaff** what? | potions |
| `r` | **read** what? | scrolls |
| `z` | **zap** what? | wands (the aiming reticle opens next, unless it's the wand of light) |
| `w` | **wield** what? | weapons, bows and crossbows |
| `W` | **wear** what? | armour |
| `P` | **put on** what? | rings |

Pick a row with its letter (or the direction keys and Enter) and it happens at once — no second menu. An item keeps the same letter in every one of these menus, so the potion that's `c` in your pack is `c` in the quaff menu too. If there's nothing to show, the game just says so ("You have nothing to read.") and you keep your turn.

Any run/auto-explore/travel stops dead the instant a monster comes into view, so it's safe to spam. **You cannot pass your turn** — if you press a key that isn't a real action, nothing happens and no turn is spent, but there's no "wait here" or "search" command either.


The pack
--------

`i` opens it: arrow keys or the item's own letter to select, Enter for the action menu — **Use**, **Throw**, **Drop**. The other nine pack keys above skip that menu and do one of those three directly, so `i` is for when you want to look at what you're carrying first.

- **Use** on a potion or scroll drinks/reads it immediately. On a wand, a bow, or anything else that needs a target, it opens an aiming reticle first — move it with the direction keys, Enter to fire, `Esc`/`x` to back out. Equipping/unequipping gear (wielding a weapon, wearing armour, putting on a ring) also goes through Use.
- **Throw** always opens the reticle. A dagger or spear is balanced for flight and pierces everything on its line; a mace is an improvised lump that a monster with hands can catch and use back on you; arrows and quarrels come out of the stack one at a time and hit twice as hard if you're holding the matching bow/crossbow. A shot that draws blood kicks the map exactly as much as your sword landing does; one the armour turns aside clinks and moves nothing — and out here there's no chip-damage floor to catch it, so it really does nothing at all. Anything balanced for flight (dagger, spear, arrow, quarrel) goes around armour entirely and can't be turned aside that way.
- **Drop** puts it back on the floor. Refused for anything cursed and worn — it won't come off (see Curses, below) — and refused for the Element of Yoord, which doesn't leave your hands until the game is over.

Ammunition stacks (up to 13 per slot); everything else takes its own slot. Coins never enter the pack at all — they are spent where they lie (see Coins). You have 9 slots, capped there on purpose so every slot's letter stays out of the way of the ones the game reserves for scrolling the list.


Combat
------

There's no to-hit roll and nothing ever misses outright. Every fight is two independent rolls, subtracted:

    damage = (1d[Power] + PowerBonus) − (1d[Armor] + ArmorBonus)

If that comes out zero or negative, nothing happens. `Pow.`/`Arm.` on your status line are already the totals — weapon, armour, and every ring you have on folded together into the same four numbers. The game never remembers *what kind* of item lent you a bonus; a longsword, a suit of plate, and a ring of protection are indistinguishable to the combat code.

Two rules exist only for you, never for monsters:

- **Excellent hit** — 15% of your swings roll three attack dice instead of one (summed, before armour comes off), a shot at cracking good armour that a monster's plain 1d[Power] never gets.
- **Chip damage** — even your worst possible swing still takes 1 HP off if it would otherwise have done nothing, but a hit that only exists because of this floor can never be the killing blow; it leaves a foe on 1 HP. It's called a glancing blow in the log, and it looks like one: a cold white clink where your steel skidded off, and no lurch of the map — the two things that tell a swing that got through from one that didn't. Monsters get no such floor — if a monster's roll can't beat your armour, it does nothing at all, which is the entire reason armour is worth wearing.

HP totals are small (you start at 12) — a couple of bad rolls in a row is a real threat the whole game, there's no leveling up to outgrow it.

Kill something wearing gear and each piece it had on gets its own coin-flip: heads, it lands on the corpse's tile and gets called out in the log so you know to go back for it; tails, it's destroyed with its owner.


Status effects
---------------

- **Confused** — every step has a coin-flip chance of stumbling in a random direction instead (still costs the turn, even into a wall). Running, auto-explore and travel all refuse to start while confused.
- **Fast / Slow** — you (or a monster) act twice as often, or half as often. Wands of haste/slow monster shift a creature one notch on this scale permanently; a hasted or slowed *player* loses it on the next staircase. Gear counts too: a ring of slow digestion reads `SLOW` for as long as it is on your finger, and taking it off gives the notch straight back — including back to a haste it was masking.
- **Stealthy** (`STLH`, a ring) — nothing on the floor notices you until it is two tiles away. It does not un-ring a bell: anything already hunting you because something shrieked keeps coming.
- **`PLAT` / `FORG`** (the platinum and forge coins) — the only two badges that are good news. Reach the next staircase without being hurt again and they pay; take a single point of damage and they don't. See "Coins".
- **Held** (bear trap) — pins your feet, not your fists: you can still swing at whatever's adjacent, but trying to step anywhere just thrashes against the trap, wasting the turn and tearing a point of HP.
- **Asleep** (sleeping gas) — the turn is forfeited outright, no key read, until it wears off.
- **Blind** (a potion) — the worst of them. You see the eight squares around you and nothing else, with no colour in any of them, and no monster at all — not even the one standing next to you. Auto-explore and auto-fight won't run, because as far as they're concerned there is nothing to see. The monsters can still see *you* perfectly well.
- **Paralysed** (a potion) — your tempo drops to `SLOW`, and on top of that about half the turns you have left are simply taken from you: no key is read, the monsters move, and the turn is gone.

None of these wear off with time. A staircase clears them, and so does being caught in a wand of cancellation — that is the whole list.

A monster carrying one of these shows it as a coloured background behind its glyph: dark blue for asleep or paralysed, dark green for held in a bear trap, dark cyan for held by a scroll, dark magenta for confused. Worth reading before you decide which one to fight.


Exploring the dungeon
-----------------------

You always see the 3×3 around you. Standing in a **lit room** floods the whole room into view, walls and doorways included; everywhere else is corridor sight — just what's directly ahead. About one room in ten past the first spawns dark and behaves like a corridor until a wand of light goes off in it. Monsters vanish from the map the moment they leave your sight; the floor layout itself, once seen, stays remembered (dim grey) for the rest of your time on that floor. Fog of war is **not** carried between visits — walk back up through a floor you already cleared and it's blank again, even though the walls are identical.

Traps come in six kinds and reveal themselves in one of three ways, picked at random per trap, so a room is never a solved puzzle:

- Some show the moment the tile enters your view.
- Some stay hidden until you're standing right next to them.
- Some are invisible until they actually go off.

The arrow trap's damage ignores your armour die entirely (not your armour bonus) and gets nastier the deeper you are, and the dart trap does the same *plus* permanently drains a point of Power — a ring of strength is the only protection. A trapdoor, a bear trap, sleeping gas, and a teleport fill out the rest.

**Trick shots.** A trap only bites what stands on it — so don't stand on it. Put a shot on a trap's tile instead (throw or fire anything at it), or catch it in a wand's blast (fire, cold, or any wand thrown as a grenade), and the whole mechanism lets go at once: a 3×3 burst of `2d3` that no armour of any kind reduces, *plus* the trap's own effect on everyone caught in it. Six arrows at once, gas over the whole knot of them, a trapdoor that swallows the pack. It's the best thing you can do with a corridor full of monsters and a `^` in the middle of them.

It is also completely indiscriminate, so mind where you're standing: catch yourself in your own trick shot and the log stops saying `BAM!` and starts asking `WHY!`. A burst is loud but not clairvoyant — one you can't see neither prints nor shakes.

**Traps aren't the only thing worth shooting.** Put a shot on a **coin** and it goes off too, in a burst twice as wide as a trap's — *and you get the coin.* Its effect reaches you across the room: shoot a red coin and it heals you, a gold one pays you, a platinum one makes you its promise. So a coin in the middle of a crowd is worth shooting even when you could have walked to it, and a coin you'd never reach in time is still worth something.

The effect lands before the burst does, so your own blast can't take the healing back off you. And unlike stepping on one, shooting is a decision: nobody checks whether you needed it first. Shoot a red coin at full health and you have wasted a red coin.

A wand's blast does it too — anything it covers goes off with it, traps and coins alike, and the coins pay whoever zapped.

Anything you throw can set either off, not just ammunition. Do it with a dagger, a potion, a spare ring — anything that isn't an arrow or a quarrel — and the dungeon says "Very clever."

And if you ever fire something at the **Element of Yoord** itself: it does not break, does not move, and is not spent. It answers. One wide burst where it lies, then another centred on everything that caught, then one more on one of them. The log will tell you what that is called.

**The Dungeon Lord's patience.** You get 260 turns on any one floor. Run past that and a portal opens under you and drops you a level deeper whether you were ready or not (or, once you're carrying the Element, a level *shallower* — the impatience runs the other way on the climb out). This is most of the reason the automation exists: `o` and `Tab` chew through a floor fast enough that patience is rarely the thing that gets you.

A staircase is also a rest: arriving on a new floor gives you back half your *maximum* HP (never past the ceiling) and refills your magic pool completely. A trapdoor plunge skips both — it's a fall, not a rest.

Seeds fix the *maps*, never the contents. The same seed always carves the same floor 7, every time, forever — but what's standing in it, what's lying around, and what's trapped is re-rolled fresh every time you enter, including walking back up through a floor you already cleared.


Magic
-----

A second resource pool, `Ma X/Y` on the status line, 4 points at the start of a run. It does not regenerate on its own — only a staircase refills it. Almost nothing spends it yet; treat the number as mostly a preview of mechanics still being wired in.


Items
-----

Nothing arrives identified. Every potion, scroll, wand and ring has a cosmetic disguise — a colour, a garbled title — shuffled fresh each seed, and drinking or reading one identifies *every* item of that same true type you'll ever find for the rest of the run, not just the one in your hand. Weapons, armour and launchers hide something too: whether they're plus or cursed, and by how much, until you actually wear them or read a scroll of identify.

**Curses.** 65% of the gear the dungeon drops is cursed — it can still roll a good number, the gamble isn't the stat, it's that the moment you equip it it welds on. Two scrolls get it back off: remove curse, which destroys the item rather than freeing it clean, and the matching enchantment scroll, which burns the curse away and mends the item's minus to +0 in the same breath — much the better outcome, and much the rarer luck. 25% of drops are plain, 10% are exceptional (+1 to +3, and safe to take off any time).

You start a run already equipped: +1 ring mail worn, +1 mace wielded, a +1 short bow and 13 arrows in the pack, and one potion of healing you already recognise.

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

Arrows (d4) and quarrels (d6) stack up to 13 to a slot and are useless without a matching bow or crossbow — with one, the die doubles. A bow or crossbow itself has nothing to swing with; it occupies the hand a sword would have, and clubbing someone with one is worth at most 1 damage whatever its enchantment. One is a build; a second is dead weight.

With a bow or crossbow drawn, `f` skips the pack: it grabs the first matching arrow/quarrel and opens the aiming reticle straight away, same as `i` → item → Throw but one key. `Tab` changes meaning too — instead of walking up to the nearest threat, it fires at it on the spot, refusing with "You're out of ammo." or "No clear shot." rather than ever closing the distance by hand.

### Wands

`z` zaps one. `2d3` damage on the attack wands, `2d4+1` charges rolled when a wand enters the dungeon, range 6 or 8 depending on the wand. All fourteen do something when zapped — nothing here is a dud. A few worth knowing about in advance: light needs no target at all, floods the room or passage you're standing in, and reveals any hidden trap in it; polymorph and the haste/slow-monster wands change a creature rather than just hurting it; and cancellation is the one to dread catching yourself in — it strips every magic effect off whatever it hits, and on you that's a catastrophe: every enchantment on your gear zeroes out, every unread scroll goes blank, every potion turns to water.

A thrown wand dumps its whole remaining charge at once and bursts wider and hotter than a zap does.

### Rings

Worn on a finger — you have two slots, and all twelve rings do something.

| Ring | What it does |
|---|---|
| protection | +2 on your armour roll |
| strength | +2 on your damage roll, and a dart trap can never drain your Power |
| increase damage | +2 on your damage roll |
| dexterity | +2 on anything you throw or loose |
| perception | you see everything invisible: hidden traps, phantoms, stashed items |
| maintain armor | nothing can corrode the armour you're wearing |
| regeneration | each turn, a coin flip to lift one status effect — or, with nothing to lift, to give back a point of Power a dart drank |
| stealth | nothing notices you until you're two tiles away. `STLH` on the status line |
| slow digestion | your digestion slows along with everything else about you: you are `SLOW` for as long as you wear it |
| teleportation | you have teleportitis. Roughly one turn in eighty-five, you are suddenly somewhere else on the floor — you land at the top of your own turn, so you always get to act before anything reaches you |
| aggravate monster | a flat 10% chance per action that the whole floor learns where you are |
| adornment | see below |

The **ring of adornment** is the strangest thing in the dungeon and the most valuable. Put it on and it doubles your score, tells you that you did it with style, throws eight fireworks, and disintegrates. It is worth one action and nothing else. Save it for the end.

Two of these are worth more the worse your run is going. Regeneration is the only cure for blindness or paralysis short of a staircase, and the only thing besides a potion that mends drained Power. Maintain armor matters the moment you meet an aquator (see the bestiary).

### Potions

All fifteen do something. Five of them change you permanently, which is worth knowing before you drink an unidentified one to find out what it is.

The three conditions potions may inflict — blindness, paralysis, confusion — never wear off on their own. A staircase is the cure.

### Scrolls

All fifteen do something now — except blank paper, which does nothing on purpose.

Four of them are worth knowing about before you read one to find out what it is:

  * **Enchant weapon / enchant armor** give what you're wielding or wearing a permanent +1, in a shower of orange sparks. If it was a minus it comes back to +0 in one go, curse and all — this is the only thing in the dungeon that saves a cursed item instead of destroying it. Read over an empty hand or a bare back, the sparks die.
  * **Sleep** rolls a wave of drowsiness over everything you can see and drops it for a few turns — helpless, not merely stuck. About one time in four it turns in your mouth instead and puts *you* down, which is a very bad few turns to spend next to something with teeth.
  * **Hold monster** roots everything in sight where it stands, for longer than sleep lasts. Held is not helpless: walk into its reach and it still bites. What you're buying is the room to leave, or the range to shoot from.
  * **Monster confusion** doesn't go off when you read it. It charges your hands (`GLOW` on the HUD) and waits: the next blow you actually land passes the confusion on and is spent doing it. A scrape off armour doesn't count.

The rest: remove curse frees every cursed item you're wearing, but destroys it rather than handing it back clean; vorpalize weapon brands your wielded weapon as the bane of one random species, and reading it a second time destroys the weapon instead of stacking; magic mapping reveals the floor as an animated wipe; and food detection is the plain counterpart to a potion of magic detection — it shows you every ordinary thing lying on the floor, exactly what the potion turns its nose up at. The Element of Yoord answers to both.

### Coins — the pickups

Every coin is a **pickup**: it is never carried, it works the instant you step on it, and then it's gone. A full pack is no obstacle, because there is nothing to put anywhere.

| Coin | What stepping on it does |
|---|---|
| gold | 5000 points |
| silver | 1000 points |
| red | restores 4 HP |
| blue | restores 4 `Ma` |
| rosé | clears up to 4 status effects, worst first |
| green | gives back up to 4 points of drained Power |
| platinum | the `PLAT` promise — see below |
| forge | the `FORG` promise — see below |

**A coin you can't use is a coin you don't take.** Walk over a red coin at full health and it stays where it is, silently, waiting for the fight that goes badly. Same for a blue one with a full pool, a rosé one with nothing wrong with you, a green one with an undrained arm. Auto-explore knows it too and won't detour for one it can't use yet — so a coin left behind is not a coin forgotten, and `o` will come back for it the moment it matters.

**The two promises.** The platinum and forge coins don't pay when you take them; they pay at the **next staircase**, and only if you get there **without taking another point of damage**:

  * `PLAT` — a permanent point of attack or defence *die*. Nothing else in the game grows those two numbers.
  * `FORG` — a point of *plus* on the weapon in your hand or the armour on your back, exactly as a scroll of enchantment would, curse and all.

One hit and it's off, with a line telling you so. Two floors' worth of careful play is what they cost, and they are the only permanent growth in a game that otherwise has none.


The bestiary
-------------

Twenty-six species, one letter each, sorted into three danger tiers: fodder appears from floor 1, a middle tier joins from floor 5, and the nastiest letters start showing up from floor 10 — early fodder never stops appearing, it just starts arriving in worse company. Most letters chase you on sight; a few flee instead, a few never move at all, and one staggers around at random the same way you do while confused.

A handful are worth knowing about before you meet them: several species will pick up whatever you throw at them and use it right back at you, a few are undead, a couple are outright immune to a specific element, one is permanently invisible, and exactly one is a legal target for any vorpal weapon. The rest is for you to find out on the way down.

One of them doesn't go for you at all. The **aquator** goes for your armour: every blow it lands takes a point off your suit's plus, for good. Nothing gives it back but a scroll of enchant armour — and a ring of maintain armor makes the whole creature harmless. Kill it early or take the suit off.

Claiming the Element of Yoord tears every one of those depth gates off its hinges: for the whole climb back up, every floor draws from the entire bestiary, no exceptions.


The Element of Yoord, and winning
-----------------------------------

Depth 13 has no down staircase. The Element sits where it would be. Picking it up flips the rule for the rest of the run: down is dead, up is the only way, and the 260-turn impatience now throws you *upward* through a level instead of down if you dawdle. The very last climb, out of depth 1, has to be walked on foot — a portal can never be the thing that wins the game for you.

Walking out of depth 1 carrying the Element is the win condition. Everything else is a loss, and most runs are a loss.

Nobody leaves that dungeon quietly. However you get out — the last stair, or a potion of raise level drunk on depth 1 with the Element in your pack — you do it with style: eight fireworks, and your score doubled on the way out.

Dying (and everything else)
------------------------------

Death is permanent, there is no retry. A "You die..." panel, a `--MORE--`, then a tombstone, what killed you, and your score.

Score, in full: **100 points per hit point of every creature that dies** (a goblin is a rounding error, a griffin is a thousand), **500 points per difficulty tier every time you take a staircase** — 500 a flight down to depth 3, 1000 through depth 6, and so on, paid on the way down and again on the way back up — and **treasure, the moment you pick it up**: 5000 for a gold coin, 1000 for a silver one, 25000 for the Element of Yoord itself. Then there are the two things that double the lot: a ring of adornment, and getting out alive.

**Kill more than one thing in a turn and the whole turn's killing is worth more**: +50% for each corpse past the first, applied to all of them together. Two at once is worth 1.5x, three is worth double, and a thrown wand that clears a room is worth far more than the same room cleared one swing at a time. The score line shouts `COMBO!` when it happens, and the log says "With style."

The score line shouts about everything, in fact: every payment flashes over it for a beat — `+700`, `COMBO! +2400`, `DOUBLE` — and then goes back to being a number.

Most runs end from attrition, not a single big monster: a string of fights with no healing potion left, and something ordinary finishes what a tougher fight started. The rest end by putting on an unidentified cursed ring to find out what it does, wandering one room too far past the Dungeon Lord's patience, or a trapdoor at the wrong moment.


Useful command-line flags
----------------------------

The full list, with every environment variable, is `docs/reference/cli-and-env.md`. The ones worth knowing as a player:

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
    Quaff/read/zap  q / r / z
    Equip       e        w wield, W wear, P put on
    Cancel      x or X   (never spends a turn)
    Quit        Q or X   with nothing open; asks first · Ctrl+C doesn't

    damage = (1d[Power] + bonus) - (1d[Armor] + bonus)
    15% of your swings roll 3 dice instead of 1
    your worst swing still takes 1 HP, but can't finish the kill
