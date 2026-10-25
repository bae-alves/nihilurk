# nihilurk
nihilurk is a roguelike about descending a dungeon, acquiring an item, and ascending it back again. Life is unfair and death is permanent. The game's humor is pessimistic, nihilistic and absurdist without being edgy. Rogue kind of had coherent a lore/plot blurb. This game is about thrill of the arcade!

Game plays like any terminal roguelike, but has simplified controls and a focus on movement automation (o to auto-explore; shift movement to go fast; tab to auto-fight). Crawl is built the same way, Angband has plenty automation too, so I took their lead. The focus of the gameplay is part surviving the attrition of multiple encounters, part being badass blowing up monsters. Hackin'n slashing but also managing limited hacking'n slashing ability, It's a hot mess.

## Vibe
Absurdist gorefest! The game is not really about anything. It's just fun and scoring, strategizing, and doing hallucinating combos!

## Progression
The game is a dungeon crawl with 13 levels, the last one has the Element of Yoord. Once the player character claims it, they must climb back out of the dungeon!

## Gameplay
`o o o TAB TAB TAB TAB o o...`
In this game, the player controls a nihilurk that can move on a grid; fight monsters by trying to move into their space; auto-explore; auto-fight and use many items (wearables, consumables, etc.) in pursuit of the Element of Yoord.

### What is different
From a limited corpus of terminal roguelikes I've played, these are where nihilurk gets... Different.

**Trick shots.** Rogue is a game about what you walk into. nihilurk is also a game about what you shoot and throw, and the rest of the design leans on it.

Almost everything on the floor that can go off can be set off from across the room. Put a missile on it and it goes kablooey:
- A trap. A trap only bites whoever is standing on it. Shot, it has nobody to bite, so the whole mechanism goes at once over the tiles around it, armor-proof, and then works its own effect on everyone caught.
- A coin. Coins are never carried, you step on them. Shoot one instead and it bursts like a trap. Nobody gets its effect.
- A potion lying on the floor. It shatters over the tiles around it.
- Bursts chain. Anything a burst covers that a shot could have set off goes off with it, including traps nobody has found.

The shot is the skill, and the game works to keep it one:
- Ranged only. A missile or a blast sets things off, a melee swing never does. Walking up and hitting a monster on a trap would make the read free.
- You can only aim at what you can see. A trap nobody has found is never a target. The game offers the shot before you take it: a creature standing on something you could set off is drawn on a magenta cell.
- It spends what it goes off on. Trap and coin are gone the moment they let go. The Element of Yoord is the one exception, and it is the exception to everything: a missile that lands on it is not spent, it answers with a wide burst, then a burst on everyone that one caught, then another. It is the ULTIMATE TRICK SHOT.
- It is nobody's friend. Stand a tile away from your own shot and it catches you too.

**Also different.**
- Wand grenades. Thrown, a wand spends every charge at once and bursts where it lands.
- Coins are pickups. You never carry one: you step on it and it acts, or you shoot it.
- A thrown deck bursts like a wand of light. The decks themselves work similarly to DCSS/Crawl's.
- You can play as any monster just by adding an arg (`-am dragon`) or naming yourself its species.
- The bodies are the classes: nihil, lurk, or any monster. There are no experience levels.
- Stairs are the only recovery that is not magic. You cannot rest, search or pass a turn, and nothing heals you as you walk.
- Everything but gear is known on sight: potions, scrolls, wands, rings, etc. Gear hides its plus and its curse until you wear it or identify it.
- No hunger, instead you are yanked into the next level if your time on it runs out.
- Like older versions of Rogue, the game starts at once, with no menus. Your name and your body are arguments.
- Where the classics ship a wizard mode, nihilurk has environment variables: `NIHILURK_SPAWN` and `NIHILURK_LEVEL` for its spawn API.
- The gorefest is opt-out by argument: `-nb` and `-nshake`.

**A small flex**: nihilurk is a human-friendly, moddable codebase, in Rust. `docs/` follows Diátaxis: a tutorial, how-tos, a reference and the reasoning behind it.

### User Skills
- Strategizing
- Resource Management
- Optimizing
- Tactics

### Mechanics
Turn based on an 80x22 grid, in a terminal at least 80x25. The player is the clock, nothing else in the game acts until the player spends a turn. Every player turn each monster banks energy at its own rate, 1 if it's slow, 2 if it's normal, 3 if it's quick, 4 if it's fast, and an action costs 2, so fast things act twice for every step the player takes and slow things act every other step. Effects can shift a creature along that scale, and it stays shifted.

Attacking is walking into something. Damage is (1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus), the two sides rolled independently and subtracted. Each 1d is two dice of that size averaged, rounding down, so a normal exchange swings less than the die suggests. Every piece of equipment folds into those four numbers and the combat code never learns what kind of item any of them came from, a weapon, a suit of armor and a ring all arrive as the same sort of modifier.

Two rules apply to the player and nothing else:
- Excellent hit. 15% of swings roll 3d[Power] instead of the usual, before armor is subtracted.
- Chip damage. The worst possible swing still takes a point off, but a swing that weak can never be the last one: it leaves things alive on 1 HP.

Anything that dies wearing gear rolls a separate coin flip per piece, heads it clatters onto the corpse's tile and gets announced, so the player knows to come back for it, tails it's destroyed along with its owner.

Automation covers everything that isn't a decision. o auto-explores the floor, O travels to a chosen tile, > and < walk to the stairs, tab chases auto-fights, shift and a direction runs. All of it halts the instant something is seen. The intended rhythm is a lot of o and tab and then a long pause where the player actually thinks.

Sight works the way Rogue's did. The player always sees the 3x3 around them, and standing anywhere in a lit room floods the whole room into view, its enclosing walls and the mouths of its corridors included. Everywhere else is corridor sight. The rest of the floor is remembered dim once it has been seen, and monsters drop off the map when they leave the viewshed. Some of the rooms past the starting one spawn unlit and behave like corridors until something lights them. Invisible things are only visible if you can see the invisible, otherwise they announce themselves by hitting you.

Traps reveal in three different ways at equal odds so searching a corridor is never a solved procedure. Trap damage ignores the armor die but not the armor bonus, and the damaging traps get nastier in three depth bands. A bear trap pins your feet, not your fists: you can still swing at whatever is next to you, but every step you try to take tears the leg for a point and wastes the turn.

Monsters are entities assembled from the same components the player is. They wield, wear, drink and read, which mostly means they use whatever gets thrown at them.

Spirits are non-combat interactions. They are `&`-glyph creatures that you make a deal with by walking into them. Some give away a choice of items, others trade with you if you have something they want. The game has no currency, so this is the only trade there is. Dealing with them too much, or hurting them, upsets the balance, and that makes all of them hostile. A scroll of atonement restores it.

Alignment is the meter behind that balance, from -3 to +3 between two camps of spirit: the demons pull you down, the angels and their kin pull you up. Each time a spirit leaves the floor (a deal, a kill, a polymorph, anything that removes it) you move one step toward its camp. Gaining a Helper moves you up one, and blowing one up moves you down two. Reach either end and the balance breaks. A thermometer of the word BALANCE under the status line shows where you stand, and reads BROKEN once the spirits turn. A scroll of atonement or a potion of adjustment resets it to 0.

#### Mechanics references (hardly exhaustive, please contribute)
- Crawl/DCSS: the automation focus, and decks of cards.
- NetHack: shift to go fast.
- Crypt of the NecroDancer: the patience timer, being unable to pass your turn.
- Rogue: most everything. It's a terminal roguelike after all.

### Items and power-ups
You start the run already equipped: armor worn, a weapon in hand, and in the pack a bow with arrows and a single potion, the armor, weapon and bow all +1.

Drops follow Rogue's own category odds, with coins and treats standing in for food. The shares below are rounded; `DROPS` in `models/src/world/spawn.rs` has the exact weights:

- Scrolls, about 28%. Recharging, protection and justice are half as common as the rest.
- Potions, about 25%, and some of them are punishments.
- Coins, about 12%. They buy nothing. A coin that would do nothing for you is not picked up at all; it keeps until it would.
- Armor, about 8%. Each suit has its own armor die.
- Weapons, about 8%. Within that, 45% a melee weapon, 35% a bundle of 4 to 13 missiles, 20% a launcher. Launchers are deliberately the rarest, one bow is a build and two are clutter. A launcher is worth at most 1 damage swung, however good it is, because it takes the hand a sword would have had.
- Wands, about 5%. Wands have many different effects. Attack wands deal 2d4. Every wand has 6 charges, and the range is per wand.
- Rings, about 5%. Worn, always on.
- Treats, about 5%. Thrown, never used: Use just tells you so. Throw the right treat at a monster and it eats it; half the time it becomes your boon companion, your Helper. A Helper chases and fights whatever monster you can see, comes back to your side when there is nothing to fight, and turns up next to you on every new floor, healed. Walk into it and you trade places; it can't do that to you. It has its own background color. You only get one.
- Decks, as common as rings. A deck comes with 5 random potion and scroll effects, decided and stacked when the deck is rolled. Reading the deck plays the top effect on the reader. Thrown, the deck bursts like a wand of light with a charge for each effect left, and the effects are lost.

The item system is one table per kind and a row per item, and a row is nothing but a name, a glyph and the components the thing carries into the world. A ring that guards you is not a special case anywhere, it is an item holding ArmorBonus(2), which combat already folds in for a suit of armor. A bow does not know arrows exist, it grants FireArrow, and an arrow is a thing that answers to FireArrow. Adding an item is one row and no other edit. This is the part I am smug about.

Gear rolls a quality when it spawns: 25% plain, 10% exceptional at +1 to +3, 65% cursed at anywhere from -5 to +5. A cursed item can roll better than a clean one, it simply won't come off once it's equipped, and it takes the right scroll to get out of it. The bonus lands on whichever roll the item feeds, never on the die itself, so a +3 weapon still rolls the die it came with. Rings roll the same odds with their own numbers: a ring that is a number is +2 plain, +3 exceptional and -3 to +2 cursed, so a cursed one is never the better ring, and every other ring has only the curse to hide. Items can also be MEGACURSED. And cursed items can be merged together by equipping one over the other to varying results.

Everything in the pack offers use, throw, drop and info, and the pack holds 9 things. Missiles stack up to 13 per slot, and loosed from the matching launcher they do more damage.

Magic is the second pool in the status line. Stairs refill it. Spells spend it, you cast them from a menu with Z, and you can know only a few at a time.

### Progression and challenge
Depth 13 has no down staircase, the Element of Yoord is sitting where it would be, and picking it up inverts the staircases for the rest of the run: down goes dead, up is the only direction. Once in hand it never leaves it: it cannot be thrown or dropped, and a thief that reaches for it bursts apart in gore. Taking a staircase heals a third of your max HP. A trapdoor does not.

The dungeon does the scaling. Monsters are sorted into three danger tiers and a floor rolls from every tier it has unlocked: fodder from depth 1, the mid tier from depth 5, the deepest letters from depth 10, so early fodder never stops appearing, it just starts arriving in worse company. Claiming the Element of Yoord tears that gate off its hinges: for the whole climb out every floor draws from the entire bestiary, so a dragon on floor 1 is not just possible, it is the dungeon's parting gift. The crowding budgets step in five depth bands ending at floors 3, 6, 9, 12 and 13 — the deepest floor its own worst band: 4 monster slots plus one per band, the first always filled and the rest filled at 80% rising 5 points a band to a cap of 95%, and 4 trap slots plus one per band, filled at 40% rising 10 points a band to a cap of 95%. The two damage traps climb their own coarser three bands, ending at 4, 8 and 13, a point of arrow damage and a point of dart strength-drain each.

From floor 6 to floor 12, one floor in ten is special: not Rogue's grid of rooms at all, but a floor built whole to a different idea, a castle for one. Ordinary floors have special rooms too, a dragon hoard for one. A special floor never has special rooms, and because it is part of the layout, it is still special on the way back up. No floor puts a special room around a staircase. On the floors that are one room and nothing else, the ground around the stair you arrive on is kept clear, the way a start room is. Rooms that share a wall light up one at a time, since a door shows the first step into the next room and no more. Deep water takes only swimmers, and whatever lands in it sinks with a splash, except the Element of Yoord, which will not drown: it leaps back into your hands and will burn away everything else you carry to make room.

A seed fixes the maps, not the mob. Every floor's walls are a pure function of the seed and the depth, so floor 7 is the same maze every time you set foot on it and every reload lands you back in it exactly — but the monsters, loot and traps are re-rolled on every entry, keyed to how many staircases you have taken. The trip back up is a trip through corridors you recognize, restocked with things you don't, and the fog of war is blank again each time (the save does not carry per-floor memory).

### Ways to play
- nihil, the default, who carries gear and charges (Shift + direction: close the gap and strike, at the price of taking harder blows for a turn). Aggressive and built on equipment, nihil is the place to start.
- The lurk (`-b lurk`), who has claws and fur, wears nothing but rings, is quick, starts knowing only Bide, and grows by killing. It plays slower than nihil: every fight is a decision, and with no gear to grow into, the run rides on consumables.
- Any monster, with its own body, its own spells and an empty pack.
- Endless (`-endless`): there is no Element and no way to win, the dungeon just keeps going down.
- Fast (`-fast`): the dungeon has only 6 floors, more monsters and more loot.

### Losing
Death is permanent. There is one save file and it exists so the player can stop playing, not so they can retry, and it is deleted the moment the run ends, before the game even asks for a keypress. The seed goes with it. Winning keeps the file, as clear data.

What follows is a You die... panel, a --MORE--, and the word LOSE with the name, the cause of death and the score under it. Winning gets the same panel with WIN on it; neither says anything else. The score is paid as you play rather than counted at the end: a hundred points per max hit point of everything that dies, with a multiplier for killing more than one thing in a turn; five hundred per difficulty tier every time you take a staircase; and treasure the moment it is in hand. Two things double the lot, and they are the same thing twice: putting on one particular ring, and getting out alive. The scoreboard is an arcade cabinet's, not an accountant's — it shouts what you just earned and then goes back to being a number. The ten best runs live on `-scores`.

Winning is walking out of depth 1 with the Element. Everything else is losing, and most runs lose.

### Art Style
This game is made for terminal screens and is styled like the original Rogue, with colored glyphs representing game objects.

### Technical Description
This is a game made to run on most shells and devices that run shells. It uses keyboard controls though, that might limit the hardware scope. It is made using bevy_ecs and crossterm on rust for unnecesarily peak performance.

Content is data. Every monster, item and trap is a row in a table, and the dungeon decides what turns up by drawing from those tables with a weight and a debut depth. `docs/` covers how.

### Demographics
The developer. Seriously. I made this game because I want to play it.

### Platforms and Monetization
Free and open source. It installs from crates.io, ships as prebuilt tarballs and a Windows zip. Linux is where it is played, Windows has been built and played by a tester, and macOS builds and passes its tests. There is a Ko-Fi link for anyone who wants to buy me a coffee.

### APPENDIX N - Instruction and Inspiration
This is a list of everything I took instruction and inspiration from when designing nihilurk and its codebase.

#### The Videogames
- Rogue, I guess.
- NetHack
- Dungeon Crawl Stone Soup
- Angband
- Umoria
- Brogue
- ADOM
- Wildfrost
- Crypt of the NecroDancer
- Slay the Spire
- Balatro
- Dwarf Fortress
- XCOM
- The Nier series
- Dicing Knight. from PlatineDispositif
- The Souls games made by From Software
- Darkest Dungeon
- Nuclear Throne
- The Binding of Isaac
- The Mystery Dungeon series
- Super Auto Pets
- One Way Heroics
- Spelunky
- WazHack
- Caves of Qud

#### Analog Games
- Ironsworn
- Tunnels and Trolls (1975 edition)
- Knave 2e
- Heroes of Cerulea
- OSRIC and the retroclone ecosystem, which is what made me try to make videogame retroclones

#### Fiction Books, where the absurdist gorefest part comes from
- Roadside Picnic
- Watership Down
- The Cosmere books by Brandon Sanderson
- Sword Art Online Aincrad, Alicization and Progressive; Danmachi; Dungeon Crawler Carl and sequels; other dungeon book series I haven't read but surely could be there

#### Technical Literature, if it's to be believed bae can read any of that
- Herbert Wolverson, *Roguelike Tutorial - In Rust*.
    - Actually, nihilurk was the first thing I did after chewing through it.
- Donna Haraway, *A Cyborg Manifesto*.
- Sal Restivo, *Red, Black, and Objective*: Science, Sociology, and Anarchism.
- Stuart Russell; Peter Norvig, *Artificial Intelligence*: A Modern Approach.
- Daniele Procida, *Diátaxis*: A systematic approach to technical documentation authoring.