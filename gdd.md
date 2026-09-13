# Roog
Roog is a classic roguelike about descending a dungeon, acquiring an item, and ascending it back again. Life is unfair and death is permanent. The game's humor is pessimistic, nihilistic and absurdist without being edgy, the game gets weirder the more you play, but the game has most everything the original Rogue had. Except a storyline. Rogue kind of had coherent a lore/plot blurb. This game is about thrill of the arcade!

Game plays like any classic roguelike, but has simplified controls and movement automation (o to auto-explore; shift movement to go fast; tab to auto-fight). The focus of the gameplay is part surviving the attrition of multiple encounters, part being badass blowing up monsters. Hackin'n slashing but also managing limited hacking'n slashing ability, It's a hot mess.

## Vibe
Absurdist wonderland gorefest! The dungeon is only normal on the surface, getting madder and madder the more the player descends. The game is not really about anything. It's just fun and scoring and strategizing, doing hallucinating combos!

## Progression
The game is a dungeon crawl with 13 levels, the last one has the Element of Yoord. Once the player character claims it, they must climb back out of the dungeon!

## Gameplay
`o o o TAB TAB TAB TAB o o...`
In this game, the player controls a roog, that can move on a grid; fight monsters by trying to move into their space; auto-explore; auto-fight and use many items (wearables, consumables, etc.) in pursuit of the Element of Yoord.

### User Skills
- Strategizing
- Resource Management
- Optimizing
- Tactics

### Mechanics
Turn based on an 80x22 grid. The player is the clock, nothing else in the game acts until the player spends a turn. Every player turn each monster banks energy at its own rate, 1 if it's slow, 2 if it's normal, 4 if it's fast, and an action costs 2, so fast things act twice for every step the player takes and slow things act every other step. Wands of haste monster and slow monster shift a creature one notch along that scale and it stays shifted.

Attacking is walking into something. There is no to-hit roll and nothing ever misses. Damage is (1d[Power] + PowerBonus) - (1d[Armor] + ArmorBonus), the two sides rolled independently and subtracted. Every piece of equipment folds into those four numbers and the combat code never learns what kind of item any of them came from, a long sword, a suit of plate mail and a ring of protection all arrive as the same sort of modifier. HP totals are Rogue's micro-HP, small enough that a goblin with a good roll ends a run that had a plan.

Two rules apply to the player and nothing else:
- Excellent hit. 15% of swings roll 3d[Power] instead of 1d[Power], before armor is subtracted.
- Chip damage. The worst possible swing still takes a point off, but a swing that weak is capped so it can never take the last one. It leaves things alive on 1 HP.

Anything that dies wearing gear rolls a separate coin flip per piece, heads it clatters onto the corpse's tile and gets announced so the player knows to come back for it, tails it's destroyed along with its owner.

Automation covers everything that isn't a decision. o auto-explores the floor, O travels to a chosen tile, > and < walk to the stairs, tab auto-fights whatever is adjacent, shift and a direction runs. All of it halts the instant something enters the viewshed. The intended rhythm is a lot of o and tab and then a long pause where the player actually thinks.

Sight works the way Rogue's did. The player always sees the 3x3 around them, and standing anywhere in a lit room floods the whole room into view, its enclosing walls and the mouths of its corridors included. Everywhere else is corridor sight. The rest of the floor is remembered dim once it has been seen, and monsters drop off the map when they leave the viewshed. A tenth of the rooms past the starting one spawn unlit and behave like corridors until a wand of light goes off in them. Invisible things are only visible with see-invisible up, otherwise they announce themselves by hitting you. Blood stains the floor under the fighting and stays there for the rest of the run.

Six traps: trapdoor, bear trap, sleeping gas, teleport, arrow, dart. They reveal in three different ways at equal odds so searching a corridor is never a solved procedure. Trap damage ignores the armor die but not the armor bonus, and the arrow and dart get nastier in three depth bands. A bear trap pins your feet, not your fists: you can still swing at whatever is next to you, but every step you try to take tears the leg for a point and wastes the turn.

Monsters are ECS entities assembled from the same components the player is, off a bestiary table of 27 rows. They wield, wear, drink and read, which mostly means they use whatever gets thrown at them.

Also, **you cannot pass your turn**.

### Items and power-ups
Nothing you find arrives identified. At world creation the seed shuffles a set of cosmetic appearances across the real potion, scroll, wand and ring types, and knowledge is stored against the effect rather than the entity, so identifying one bubbly potion identifies every bubbly potion anywhere, forever. Learning what a thing does usually costs HP.

You start the run already equipped: +1 ring mail worn, +1 mace in hand, and in the pack a +1 bow with 26 arrows, a wand of magic missile, and a single potion of healing you already know the shape of.

Drops follow Rogue's own category odds, with coins standing in for food:

- Scrolls, 30%. Magic mapping, enchant weapon, enchant armor, remove curse, teleport, aggravate monster, scare monster, create monster, vorpalize weapon.
- Potions, 27%. Healing, extra healing, haste self, gain strength, restore strength, see invisible, monster detection, magic detection, raise level, and the ones that are punishments: confusion, paralysis, poison, blindness.
- Coins, 17%. They buy nothing and they are never carried: a coin is a pickup, spent the instant you step on it. Two of the eight are treasure and go straight into the score; the rest are a small mercy — four HP, four magic, four status effects cleared, four points of drained strength — and two are promises the next staircase keeps if you reach it unhurt, paying the only permanent growth in the game. A coin that would do nothing for you is not picked up at all; it keeps until it would. And a coin can be shot instead of stepped on — twice a trap's burst, and the coin's effect reaches whoever set it off from wherever they are standing, which turns every `$` on the floor into a grenade with a benefit attached.
- Armor, 8%. Leather through plate mail, an armor die of 2 up to 9.
- Weapons, 8%. Within that, 45% a melee weapon (dagger d4, spear d6, mace d6, long sword d8, two-handed sword d10), 35% a bundle of 3 to 12 arrows or quarrels, 20% a bow or crossbow. Launchers are deliberately the rarest, one bow is a build and two are clutter. A bow or crossbow is worth at most 1 damage swung, however good it is, because it takes the hand a sword would have had and you can never pass a turn to swap back.
- Wands, 5%. Light, striking, lightning, fire, cold, magic missile, polymorph, haste monster, slow monster, drain life, teleport away, teleport to, cancellation, nothing. 2d3 damage on the offensive ones, 2d4+1 charges, and the range is per wand, 6 or 8.
- Rings, 5%. Worn, always on. Protection is +2 armor, strength is +2 power plus immunity to strength drain, perception reveals invisible things, dexterity is +2 on throws, aggravate monster is a 10% chance per action of waking the floor up.

The item system is one table per kind and a row per item, and a row is nothing but a name, a glyph and the components the thing carries into the world. A ring of protection is not a special case anywhere, it is an item holding ArmorBonus(2), which combat already folds in for plate mail. A bow does not know arrows exist, it grants FireArrow, and an arrow is a thing that answers to FireArrow. Adding an item is one row and no other edit. This is the part I am smug about.

Gear rolls a quality when it spawns: 25% plain, 10% exceptional at +1 to +3, 65% cursed at anywhere from -5 to +5. A cursed item can roll better than a clean one, it simply won't come off once it's equipped, and it takes a scroll of remove curse (which destroys it) or the matching scroll of enchantment (which lifts the curse and mends the minus) to get out of it. The bonus lands on whichever roll the item feeds, never on the die itself, so a +3 dagger is still a dagger.

Everything in the pack offers use, throw and drop. Throws are aimed. A dagger or a spear is balanced for flight and pierces the whole line instead of stopping at the first body, a mace is an improvised lump that gets blunted by armor and can be caught out of the air and used back. Ammunition stacks up to 26 per slot and doubles its die when thrown by someone holding the matching launcher, d4 to d8 for an arrow out of a bow.

Magic is the second pool in the status line, 4 points at the start of a run. It does not regenerate. Stairs refill it.

### Progression and challenge
Thirteen floors down, then thirteen back up. Depth 13 has no down staircase, the Element of Yoord is sitting where it would be, and picking it up inverts the staircases for the rest of the run: down goes dead, up is the only direction.

There are no experience levels and no skill tree. The player does not get stronger, the kit gets stronger and the player gets better at reading the floor, which is a different thing.

The dungeon does the scaling. Monsters are sorted into three danger tiers and a floor rolls from every tier it has unlocked: fodder from depth 1, the mid tier from depth 5, the deepest letters from depth 10, so early fodder never stops appearing, it just starts arriving in worse company. Claiming the Element of Yoord tears that gate off its hinges: for the whole climb out every floor draws from the entire bestiary, so a dragon on floor 1 is not just possible, it is the dungeon's parting gift. The crowding budgets step in five depth bands ending at floors 3, 6, 9, 12 and 13 — the deepest floor its own worst band: 3 monster slots plus one per band, filled at 60% rising 12 points a band to a cap of 95%, and 4 trap slots plus one per band, filled at 12% rising 13 points a band to a cap of 75%. The two damage traps climb their own coarser three bands, ending at 4, 8 and 13, a point of arrow damage and a point of dart strength-drain each.

A seed fixes the maps, not the mob. Every floor's walls are a pure function of the seed and the depth, so floor 7 is the same maze every time you set foot on it and every reload lands you back in it exactly — but the monsters, loot and traps are re-rolled on every entry, keyed to how many staircases you have taken. Walking the Element of Yoord back up the thirteen floors is a trip through corridors you recognise, restocked with things you don't, and the fog of war is blank again each time (the save does not carry per-floor memory).

The Dungeon Lord allows 260 turns a floor. Past that a portal opens under the player and drops them one level deeper whether or not they were ready, which is most of the reason the automation exists. On the climb out the same impatience works the other way and the portal throws them up instead. The final stair out of depth 1 has to be climbed on foot, a portal can never be the thing that wins the game.

### Losing
Death is permanent. There is one save file and it exists so the player can stop playing, not so they can retry, and it is deleted the moment the run ends, before the game even asks for a keypress. The seed goes with it. Winning keeps the file, as clear data.

What follows is a You die... panel, a --MORE--, and a tombstone with the name, the cause of death and the score. The score is paid as you play rather than counted at the end: a hundred points per hit point of everything that dies, with a multiplier for killing more than one thing in a turn; five hundred per difficulty tier every time you take a staircase; and treasure the moment it is in hand. Two things double the lot, and they are the same thing twice: putting on a ring of adornment, and getting out alive. The scoreboard is an arcade cabinet's, not an accountant's — it shouts what you just earned in colour and then goes back to being a number.

Most runs end in attrition rather than in a boss. Four fights in a row with no healing left and then a kestral finishes what a troll started. The others end by putting the unidentified ring on to find out what it was, by exploring one room past the Lord's patience, or through a trapdoor.

Winning is walking out of depth 1 with the Element. Everything else is losing, and most runs lose.

### Art Style
This game is made for terminal screens and is styled like the original Rogue, with colored glyphs representing game objects.

### Technical Description
This is a game made to run on most shells and devices that run shells. It uses keyboard controls though, that might limit the hardware scope. It is made using bevy and crossterm on rust for unnecesarily peak performance.

Content is data. Every monster, item and trap is a row in a table, and the dungeon decides what turns up by drawing from those tables with a weight and a debut depth, so adding a thing is usually adding a line. `docs/` covers how: a tutorial, a recipe per kind of content, a reference for every field, and the reasoning behind the shape. This document is the design; `docs/` is the code.

### Demographics
The developer. Seriously. I made this game because I want to play it.

### Platforms and Monetization
I'll put it ou AUR with a Patreon Link

### Localization
English, Portuguese and Spanish. A classic roguelike in non-English is important to exist. Haitian Creole planned.

### Other ideas/Expansion backlog
- More player character options
- Leaderboards
- Tournament play
- Steam (?) + Achievements
- Nethack bones but instead of a ghost, it's the actual past failed character coming for you. A user can just rm the bones and that is okay but would be missing out on past run loot (all wearables cursed). Failed characters will be really angry about you failing them and will spout markov-chain angry nonsense. If they share names with the current character the failed character will also be treated as 'you'. This whole shebang won't happen all the time.
- Rarely, log lines are in Spanish even if you choose something else as language
- Forgotten beasts like in Dwarf Fortress. Sometimes they die in one hit, sometimes they are literally invincible. The description pop-up will tell which one is which
- The Nemelex decks from DCSS but the cards will always log Yu-gi-oh references and generally be sillier
- Trauma bonding with The Dungeon Lord because the entire game is actually a metaphor for abusive relationships. You can't seem to stop getting your head bashed in (seamlessly baked into the combat math already)
- Game log has *the hots* for Fidel Castro. It will be hard getting into a situation where this is relevant. But it will be there somewhere.
- There will be also a markov chain with Voltaire (philosopher) and Voltaire (musician) quotes. They will also be part of the mad logging.