nihilurk: instruction manual
========================

Descend thirteen floors, take the Element of Yoord, and carry it back to the surface. Monsters, traps, unidentified items, and permanent death stand between you and the exit.


Starting the game
-----------------

Start a new expedition with:

    cargo run -p engine                  # start a game
    cargo run -p engine -- YourName       # name your nihilurk
    cargo run -p engine -- -s 1234        # play a particular seed

You may quit and return to one expedition later. There is one save file. It is for stopping, not for undoing a death: when an expedition ends, the save is gone. A completed expedition is kept as clear data, so beginning another game after a victory is a choice to enter the dungeon again.


The dungeon screen
------------------

The status line gives you the information you need at a glance:

    nihilurk | HP 9/12 | Ma 4/4 | Pow. 1d4+1 | Arm. 1d3+1 | DEPTH 3 | SCORE 140

`HP` is your health. `Ma` is your magic. `Pow.` and `Arm.` show your current attack and defence. `DEPTH` tells you where you are. The last field is your score, except when the dungeon flashes a recent reward or warning in its place.

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
| `) ] }` | Weapon, armour, or launcher |
| `"`    | The Element of Yoord |

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

Walk into a monster to attack it. There is no separate attack command. You cannot walk diagonally through the corner of two walls, and you cannot attack a wall. Also, **you cannot rest, search or otherwise pass your turn**.

Health is scarce. Armour can turn a blow aside, but no weapon is guaranteed to save you. If a fight is going badly, leave it, use a potion, or find another way around.

Several commands let you spend less time walking:

| Key | Action |
|-----|--------|
| Shift + direction | Run until you meet an obstacle or something worth noticing |
| `o` | Explore the floor automatically |
| `A` | Toggle picking up useful items during auto-explore |
| `O` | Choose a destination and travel there |
| `Tab` | Fight the nearest visible threat automatically |
| `f` | Fire the launcher in your hand |
| `v` | Make a reach attack with a suitable weapon |

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
| `r` | Read a scroll |
| `z` | Zap a wand |
| `w` | Wield a weapon or launcher |
| `W` | Wear armour |
| `P` | Put on a ring |

Throwing, firing, and most wands open an aiming cursor. Move it with the direction keys, press Enter to act, or press `x` to cancel. Arrows and quarrels are stronger with the matching bow or crossbow in your hand. A thrown dagger or spear can pass through more than one target; other things may stop at the first creature they hit.

Your pack has limited space. Ammunition shares a slot with matching ammunition, but other items take their own place. Coins are not carried: stepping on one spends it immediately. If a coin cannot help you yet, the dungeon leaves it where it is.


Unknown things
--------------

Potions, scrolls, and wands tell you exactly what they are the moment you find them.

Weapons, armour, and rings hold back one thing: their own quality. An item may be plain, unusually good, or cursed, and you cannot tell which just by picking it up — though a ring always tells you what it does. Wearing it settles the question; so does a scroll of identify, read before you commit. A cursed item may be powerful, but once you put it on, it may refuse to come off. Keep a way to remove a curse before testing something you cannot afford to lose.

Magic and moves
---------------

The `Ma` pool powers your special moves. Stairs refill it; it does not return merely because you wait. Press `Z` to open the moves menu and pick a move by its row letter.

Heroic mana teaches a new move when you step on it. You can know only a few moves at once. A move that needs a target uses the same aiming cursor as a wand.

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
| `Z` | Open the moves menu |
| `x` | Close the current menu or cursor |
| `X` | Close the current menu; with nothing open, quit |
| `Q` | Quit after confirmation |
| `Ctrl+C` | Quit immediately |

`Esc` backs out of menus. It is not a quit key. Quitting saves the current expedition unless saving has been disabled.

The map can be centred, screen shake can be disabled, and blood can be hidden with command-line options. For the complete list, see `docs/reference/cli-and-env.md`.


Quick reference
---------------

    Move          arrows / hjkl / numpad; diagonals yubn or 7913
    Attack        walk into a monster
    Run           Shift + direction
    Explore       o        A toggles pickups
    Travel        O, steer, Enter
    Auto-fight    Tab
    Fire          f        with a bow or crossbow drawn
    Look          ;        steer the cursor, no turn spent
    Stairs        > down, < up (or . and ,)
    Pack          i
    Use           a        throw t        drop d
    Quaff         q        read r          zap z
    Equip         e        wield w         wear W        ring P
    Moves         Z        pick a slot by its row letter
    Cancel        x or X   never spends a turn
    Quit          Q or X   with nothing open; Ctrl+C skips confirmation
