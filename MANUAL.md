nihilurk: instruction manual
========================

Descend thirteen floors, take the Element of Yoord, and carry it back to the surface. Monsters, traps, gear of unknown quality, and permanent death stand between you and the exit.

If you have played other roguelikes, this table shows where nihilurk parts from them:

| You may expect | Here |
|----------------|------|
| Resting and searching | You cannot rest, search or pass a turn. Stairs are the only recovery that is not magic. |
| A food clock | No hunger. The Dungeon Lord's patience runs out floor by floor instead. |
| Experience levels | None. e.g. Nihil gets stronger by finding things and the lurk by killing them. |
| Unidentified items | You know what each one is the moment you find it. *Only gear hides its quality*. |
| Shops | Spirits give items away or trade them. There is no money. |
| A start-up menu | None, as in Rogue. Your name and your body are arguments. |
| Pets | You can only have one Helper pet that follows you around, other charmed creatures won't carry through floors |
| Wands | You can zap one, or throw it to spend every charge at once. |
| Gold | A coin is spent the moment you step on it, or you can shoot it. |


Starting the game
-----------------

If you built the game using cargo, start a new expedition with:

    cargo run -p nihilurk                  # start a game
    cargo run -p nihilurk -- YourName       # name your nihilurk
    cargo run -p nihilurk -- -s 1234        # play a particular seed
    cargo run -p nihilurk -- -b lurk        # descend as a lurk
    cargo run -p nihilurk -- -am dragon     # descend as any monster in the dungeon

nihilurk needs a terminal of at least 80 columns by 25 rows. In a smaller one the message log draws garbled letters. That is a known limit and will not be fixed.

The commands above run the game from a clone. After `cargo install nihilurk`, type `nihilurk` in their place. If your shell cannot find it, `~/.cargo/bin` is not on your `PATH`; the README's install steps shows how to add it.

Your name, if you give one, comes first. A name cannot start with `-`; an argument that does and is not a flag is refused. Everything else comes after it.

Other flags, in any order after the name:

| Flag | Effect |
|------|--------|
| `-c`, `--centered` | Centre the map on you instead of using the fixed viewport. |
| `-ns`, `--no-save` | Write no save file. |
| `-nb`, `--no-blood` | No bloodstains, no corpse-and-bones death animation. |
| `-nshake`, `--no-shake` | No screen shake. |
| `-nobones`, `--no-bones` | A death writes no bones file and an ascent reads none. |
| `-endless`, `--endless` | No Element of Yoord spawns, so you cannot win. The dungeon keeps going down. |
| `-anim-rate N`, `--anim-rate N` | Scale animation hold time, 0.1 to 5.0. Raise it if your terminal redraws slowly. |
| `-content`, `--content` | List every name `NIHILURK_SPAWN` accepts, and exit. |
| `-scores`, `--leaderboard` | Print the ten highest scores, and exit. |
| `-h`, `-help`, `--help` | Print a short guide, and exit. |

nihilurk speaks English and a bit of Portuguese (`pt`). Spanish (`es`) and Haitian Creole (`ht`) exist in the codebase but need help, so please contribute! The installed `nihilurk` command picks the language from `LC_ALL`, `LANG` or `LANGUAGE`, and falls back to English if none matches. To choose one yourself, put `--lang` first:

    nihilurk --lang pt

From a clone, a language is a build choice and there is no flag:

    cargo run -p nihilurk --no-default-features --features lang-pt

Other roguelikes keep a wizard mode for testing. nihilurk has three environment variables instead. `NIHILURK_SPAWN` drops the things you name around you on every floor, `NIHILURK_LEVEL` makes every floor one kind of special level, and `NIHILURK_MAGICMAP` picks the magic mapping animation. The list is in `docs/reference/cli-and-env.md`.

You may quit and return to one expedition later. Saves are for stopping, not for undoing a death: when an expedition ends, the save is gone. A completed expedition is kept as clear data, so beginning another game after a victory is a choice to enter the dungeon again.


Who goes down
-------------

Two creatures were written to go down there, and `-b` picks between them.

**Nihil** is the default, and the rest of this manual describes nihil's
expedition: twelve hit points, four magic, and a mace, a bow and a suit of
ring mail to start with. Everything nihil is worth in a fight is something
nihil is carrying, which means everything can be improved, and everything can
be lost.

Nihil plays forward. Every sword, suit and ring you find goes on and makes you
better at the one thing nihil does well, which is walk up to trouble and hit
it. That makes nihil the body to start with: the choices are few and plain,
and a bad one costs you a fight, not the run.

Nihil is born knowing one trick, the **charge**. Hold Shift and press a
direction with a creature two to six tiles off that way, and nihil crosses the
gap in one turn and strikes. The way there has to be straight and clear: open
floor, no creature in between. The price is that until the creatures have had
their turn, every blow they land on you is 25% harder (`VULN` on the status
line). You cannot charge with a foe already next to you; you are in the thick
of it, and nihil knows better. A trap on the tile you land on goes off; a trap
you jump over does not.

**The lurk** (`-b lurk`) is quadruped, fanged, clawed and furred, and shows on
the map as a magenta `@`. It is the other way of playing:

  * Seven hit points and two magic. It is thinner than nihil in every way that
    can be measured at the start.
  * It carries nothing and can put nothing on but rings. The claws are the
    weapon and the fur is the armour, so no sword and no breastplate will ever
    add to either.
  * It is **quick** — half again as fast as anything else in the dungeon,
    marked `QUIK` on the status line. Three moves for every two the floor
    gets.
  * It moves quietly, the way a ring of stealth does: nothing notices it until
    it is within reach.
  * It fights like a fencer without a blade. Step toward a creature with one
    empty tile between you and the lurk lunges across it: a sure hit at triple
    the roll, armour ignored. Every blow that lands winds the next one up.
    Shift still runs, but it cannot charge; that is nihil's.
  * It knows Bide, and pays magic for it like anybody else.
  * **It eats and grows.** Any creature that dies on the floor may feed it:
    roughly one in seven does, and the lurk gains a point of health, magic,
    attack or defence, at random. `FEAR THE LURK!` There is no counter to
    watch and nothing to save up for -- it happens while you hunt or it does
    not.

Where nihil gets stronger by finding things, the lurk gets stronger by killing
things. A lurk that avoids fights stays a lurk that can be killed by a bat.

The lurk plays slower than nihil, though it moves faster. With seven hit points
it cannot trade blows, so every fight is a question: lunge or wait, bide or
step away, drink the potion now or keep it. Nothing it finds can be worn but a
ring, so the potions, scrolls, wands and coins carry the run, and each one is
spent once. Pick the lurk once nihil's dungeon feels familiar.

The dungeon screen
------------------

Two lines carry everything you need at a glance. Above the map:

    DEPTH 3                              SCORE 000140

and below it:

    BAE · HP 9/12 · Ma 4/4 · Pow. 8+1 · Arm. 5+1

`DEPTH` tells you where you are. `SCORE` is your score, except when the dungeon flashes a recent reward or warning in its place.

`HP` is your health and `Ma` is your magic, each shown as what you have out of what you can hold. `Pow.` and `Arm.` are your attack and your defence: the first number is the die that gets rolled -- `8` means a roll of one to eight -- and anything after it is added to that roll every time, so `8+1` is two to nine. A cursed item shows as a minus. `Pow.` turns green when something has poisoned your strength below its usual ceiling.

Short words may follow, one for each thing currently true of you -- `FAST`, `QUIK`, `SLOW`, `STLH` for moving unnoticed, and one apiece for confusion, blindness and the rest.

The message log below the map describes what has just happened. When `--MORE--` appears, press Space or Enter to continue reading. Until you do, the rest of the dungeon is waiting for you.

The map uses these symbols:

| Symbol | Meaning |
|--------|---------|
| `.`    | Room floor |
| `▒`    | Corridor |
| `#`, `|` | Wall |
| `+`    | Door |
| `>`    | Stairs down |
| `<`    | Stairs up |
| `@`    | You |
| A letter | A monster |
| `%`    | A corpse |
| `^`    | A discovered trap |
| `$`    | A coin or other pickup |
| `! ? / =` | Potion, scroll, wand, or ring |
| `'`    | A rune: a scroll that goes dark instead of crumbling, and wakes when you take the stairs |
| `) ] }` | Weapon, armour, or launcher |
| `"`    | The Element of Yoord |

A monster that cannot fight at full strength is highlighted with a different tint. Dark blue is asleep. Cyan is paralysed: it loses half its turns. Dark green is held fast by a bear trap or a scroll of hold monster. Yellow is confused or fleeing. Grey is slowed.

Grey tiles are places you have seen but cannot currently see. The dungeon remembers walls and corridors, but not the creatures hiding beyond your sight.

Moving and fighting
-------------------

nihilurk is turn-based. Your action gives the dungeon its turn. You cannot pass a turn, so choose an action whenever you press a key.

Move one square at a time with the arrows, vi keys, or number pad:

| Direction | Arrows | vi keys | Numpad |
|-----------|--------|---------|--------|
| North     | Up     | `k`     | `8`    |
| South     | Down   | `j`     | `2`    |
| West      | Left   | `h`     | `4`    |
| East      | Right  | `l`     | `6`    |
| Northwest |        | `y`     | `7`    |
| Northeast |        | `u`     | `9`    |
| Southwest |        | `b`     | `1`    |
| Southeast |        | `n`     | `3`    |

Walk into a monster to attack it. There is no separate attack command. You cannot walk diagonally through the corner of two walls, and you cannot attack a wall. Nothing heals you as you walk.

Health is scarce. Armour can turn a blow aside, but no weapon is guaranteed to save you. Now and then you land an excellent hit, and it shatters whatever armour your foe has on. Cursed armour bursts into splinters of evil magic that hurt the monsters around it and spare you and your allies. If a fight is going badly, leave it, use a potion, or find another way around.

Spirits are the shops. They are `&` creatures, and you make a deal by walking into one. Some give away a choice of items. Others trade if you carry something they want. Deal with them too much, or hurt them, and you upset the balance: every spirit turns hostile. A scroll of atonement makes them peaceful again.

You also carry a hidden alignment, from -3 to +3, between two camps of spirit. The yellow, red, blue and pink demons pull you down. The angel, the sphynx, sylphids, salamanders, undyne and gnomes pull you up. Each time a spirit leaves the floor (a deal done, a kill, a polymorph, anything that removes it) your alignment moves one step toward its camp. Gaining a Helper moves you up one, so a pink demon who joins you pulls you down one and up one. Blowing one up moves you down two, whether a new Helper replaces it or system shock bursts it. Reach +3 or -3 and you upset the balance. Off centre, the word BALANCE shows under the status line, left of the map, as a thermometer: one of its seven letters is tinted for your level, red for the demons' side and cyan for the angels', the rest white. When the spirits turn, it reads BROKEN instead, in the colour of the side you leaned to (white if a wound broke it while you were centred). A scroll of atonement and a potion of adjustment both bring it back to 0.

Several commands let you spend less time walking:

| Key | Action |
|-----|--------|
| Shift + direction | Run until you meet an obstacle or something worth noticing; any body can run. As nihil, with a creature in view that way: CHARGE! Close the gap and strike, but take 25% more from melee until they have moved |
| `o` | Explore the floor automatically |
| `A` | Toggle picking up useful items during auto-explore |
| `O` | Choose a destination and travel there |
| `Tab` | Fight the nearest visible threat automatically; throws a boomerang or moon blade in your hand |
| `f` | Fire the launcher in your hand, or throw the boomerang or moon blade in it |
| `v` | Make a reach attack with a suitable weapon, or throw the boomerang or moon blade in your hand |
| `F1` | Show the key list |
| `F2` | Hide or show the yellow key hints at the bottom left |

Automation stops when a monster enters view. Auto-fight will not start when you are badly hurt or confused. Treat these commands as a way to handle safe ground, not as a substitute for looking at the screen.


Using your pack
---------------

Items you find are picked up automatically when there is room. Press `i` to open the pack. Select an item with its letter, the direction keys, and Enter. An item keeps the same letter in every item menu.

These commands go directly to the relevant kind of action:

| Key | Action |
|-----|--------|
| `a` | Use an item |
| `t` | Throw an item |
| `d` | Drop an item |
| `e` | Equip something you can wear or wield |
| `q` | Quaff a potion |
| `r` | Read a scroll or a rune |
| `z` | Zap a wand |
| `w` | Wield a weapon or launcher |
| `W` | Wear armour |
| `P` | Put on a ring |

Throwing, firing, and most wands open an aiming cursor. Move it with the direction keys, press Enter to act, or press `x` to cancel. Arrows, quarrels and blowdarts are stronger, and carry twice as far, with the matching bow, crossbow or blowgun in your hand; a blowdart saps the power of whatever it wounds. Anything thrown that was not made for throwing does 1d2, and armour a creature wears (its plus) always takes some of the damage off. Small things — potions, scrolls, wands, rings — fly further out of a bare hand than a spear or a sack of arrows does. A thrown dagger or spear can pass through more than one target; other things may stop at the first creature they hit. A thrown boomerang hits the first creature in its way and flies back to you: to its old place in your pack, and into your hand if you were wielding it. With one in hand, `f`, `v` and `Tab` throw it.

Your pack has limited space. Ammunition shares a slot with matching ammunition, but other items take their own place. Coins are not carried: stepping on one spends it immediately. If a coin cannot help you yet, the dungeon leaves it where it is.

**Trick shots.** A trap has nobody to bite when nobody is standing on it, so a missile that lands on one sets the whole mechanism off at once, over every square around it. A coin shot the same way bursts just as far and gives its effect to you from across the room. You can only do this to a trap you have already found. A creature standing on a trap you know about, on a coin, or on the Element of Yoord is drawn on a magenta square: hit it and you set off what it is standing on. One burst sets off anything it covers, including traps nobody has found, so a good shot can run a long way. None of this is on your side. Stand too close to your own trick shot and it will catch you as readily as anything else.


Decks of cards
--------------

A deck of cards is as rare as a ring. It holds five cards, stacked when the deck turns up, and one or two of them lie reversed. Read it (`r`) and you play the top card; a reversed card plays its darker side. Some cards play other cards. Throw the deck instead and all that is left plays at once as a poker hand, never reversed, on you. Every card that makes up the hand pays 5,000 points, and a FOOL counts as any card. Five of a kind plays its card five times, then sets everything you wear to +5 and burns off its curses.

Helpers
-------

A treat is food for a creature that is not you. You cannot use one, so throw it. Throw the right treat at a monster and it eats the treat. Half the time it becomes your Helper.

A Helper fights the monsters you can see, comes back to your side when there is nothing to fight, and turns up next to you on every new floor, healed. It has its own background colour. Walk into it to trade places. You get one Helper at a time. A second one blows up the first in a shower of gore, and that costs you alignment (see Spirits above). Wands of polymorph, haste and slow reach your Helper and other allies, not only enemies. Polymorph a Helper that is already polymorphed and it can burst from system shock, which costs you alignment too.


Thrown wands
------------

Zap a wand (`z`) and it spends one charge. Throw it (`t`) and it bursts where it lands.


Coins
-----

You do not carry coins. Stepping on one spends it on the spot, and a coin that would do nothing for you stays where it is until it would. Shoot one instead and its effect reaches you from where you stand.

  * Some coins are treasure and add to your score.
  * Some coins heal you, refill your magic, lift your afflictions, or restore drained strength, up to four points each.
  * Two coins make you a promise. Reach a staircase unhurt and it pays: the first adds a point to your attack die or your armour die, the second adds a plus to your weapon or your armour. Any blow that lands on you before then takes the promise back.
  * The hero coin teaches you a spell you do not know.


Unknown things
--------------

Potions, scrolls, and wands tell you exactly what they are the moment you find them.

Weapons, armour, and rings hold back one thing: their own quality. An item may be plain, unusually good, or cursed, and you cannot tell which just by picking it up — though a ring always tells you what it does. A ring that is a number (protection, strength, increase damage, sharpshooting) is +2 when plain, +3 when unusually good, and anywhere from -3 to +2 when cursed; the other rings have no number to hide, only the curse. Wearing it settles the question; so does a scroll of identify, read before you commit. A cursed item may be powerful, but once you put it on, it may refuse to come off. Keep a way to remove a curse before testing something you cannot afford to lose. Put on a cursed item you already know about when its slot is full and holds a cursed item, and the two merge (a free finger just takes the ring). Monsters that catch or steal cursed gear merge it too: 45% of the time the worn item survives, 45% the new one does, and 10% both break. Whatever survives takes the new item's plus and vorpal bane, replacing its own. With two cursed rings on, the first in your pack is the one that merges.

Magic and spells
----------------

The `Ma` pool powers your spells. Stairs refill it; it does not return merely because you wait. Press `Z` to open the spells menu and pick a spell by its row letter.

A hero coin teaches a new spell when you step on it. You can know only a few spells at once. A spell that needs a target uses the same aiming cursor as a wand.

A staff in your hand changes what your attacking spells are worth: each one costs more magic than usual and hits harder than usual, and it hits harder than it costs. The spells that heal, ward, reveal or steady you are untouched. Nothing on the screen shows this, so the staff says so itself -- when you take it up, and again when you put it away.

Polymorph lends you a creature's powers until the next staircase. A potion, a wand, a ring (which rolls on its own) and two spells, Polymorph Self and Polymorph Other, all do it. You keep your own face and numbers. A shape with no hands cannot hold gear: yours comes off and stays off. `POLY` on the status line shows it. Polymorph something that is already polymorphed and it is a coin flip: system shock, where a monster bursts and you are left on 1 HP, or a chimera, a typhon or an echidna. The ring never shocks. A ring of sustain form turns every polymorph aside, shock included.

Some effects change how you act. Confusion makes movement unreliable. Blindness limits what you can see. Paralysis and sleep steal time. A medusa's gaze steals them too, by turning you to stone -- but stone is hard: while it lasts nothing gets more than a chip through you and nothing can take your last point of health, unless what is standing over you is swinging a war hammer. Haste makes you quicker; slow makes you slower. A staircase clears these effects.


Exploring the floors
--------------------

You can always see the squares immediately around you. A lit room reveals its whole interior; corridors reveal much less. Monsters disappear from the map when they leave your sight, even though the floor remains in your memory.

Traps may announce themselves, wait until you approach, or remain hidden until they spring. A discovered trap is marked with `^`. Do not assume that an empty-looking corridor is safe merely because the last one was.

The Dungeon Lord allows only so much hesitation on each floor. If you stay too long, a portal carries you to another depth whether you are ready or not. The automatic movement commands exist partly to help you cross ground that has already proved safe.

Reaching a new floor restores some health and all of your magic. A trapdoor is not a rest; it is a fall.

The seed fixes the shape of the floors, but not everything you will meet there. A familiar corridor may hold unfamiliar monsters, traps, and treasure when you return.


The descent and the return
--------------------------

Take the down stairs to reach deeper floors. On the thirteenth floor there is no staircase down. The Element of Yoord waits there instead.

When you take the Element, you must climb instead of descend. The creatures you meet may be drawn from anywhere in the dungeon.

To win, carry the Element up and walk out of the dungeon from the first floor. A portal can move you between floors, but it cannot win the expedition for you.

Death is permanent. There is no retry or resurrection.


Useful commands
---------------

| Key | Action |
|-----|--------|
| `>` or `.` | Go down or walk to the down stairs |
| `<` or `,` | Go up or walk to the up stairs |
| `;` | Look around: move the cursor over any tile in view to have it described, including what a monster is dangerous for |
| `Z` | Open the spells menu |
| `x` | Close the current menu or cursor |
| `X` | Close the current menu; with nothing open, quit |
| `Q` | Quit after confirmation |
| `Ctrl+C` | Quit immediately |

`Esc` backs out of menus. It is not a quit key. Quitting saves the current expedition unless saving has been disabled. While time is stopped nothing is saved: the game saves as time stops and again as it starts. Quitting in between warns you in red and asks a second time, and those turns are lost.

The map can be centred, screen shake can be disabled, and blood can be hidden with command-line options. For the complete list, see `docs/reference/cli-and-env.md`.


Quick reference
---------------

    Move          arrows / hjkl / numpad; diagonals yubn or 7913
    Attack        walk into a monster
    Run           Shift + direction
    Explore       o        A toggles pickups
    Travel        O, steer, Enter
    Auto-fight    Tab
    Fire          f        with a bow, crossbow or blowgun drawn
    Reach         v        with a reach weapon
    Look          ;        steer the cursor, no turn spent
    Stairs        > down, < up (or . and ,)
    Pack          i
    Use           a        throw t        drop d
    Quaff         q        read r          zap z
    Equip         e        wield w         wear W        ring P
    Spells        Z        pick a slot by its row letter
    Cancel        x or X   never spends a turn
    Quit          Q or X   with nothing open; Ctrl+C skips confirmation
