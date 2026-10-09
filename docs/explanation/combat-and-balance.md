Why monsters are so fragile
===========================

    Audience       Anyone choosing numbers for a new monster or a new
                   piece of gear and wondering what "good" looks like.
    Prerequisites  None. Helps to have read a bestiary row.
    This is        Understanding, not instructions. For the numbers
                   themselves see `../reference/content-tables.md`; for
                   the act of adding a row, `../how-to/add-a-monster.md`.

A kestral has three hit points. That is not a placeholder.


One exchange, two dice
----------------------

nihilurk resolves a fight as two independent rolls, subtracted:

    damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)

Both sides roll every time. There is no to-hit roll and nothing ever misses -- a swing that fails is not a miss, it is a hit the armour ate.

Each `1d` there is really two dice of that size averaged, rounding down (`BELL_CURVE_DICE`). The range is the plain die's; the spread is narrower, and the rounding shades the mean down by about a quarter point. An excellent hit, below, rolls its dice flat.

That is the melee formula. Damage comes in three kinds, and armour answers each differently:

    Melee    armour die + armour plus   (the formula above)
    Missile  armour plus only           (every throw, every shot)
    Magic    nothing                    (wands, spells, blasts)

Missiles go around the armour die because a point already in flight does not care what you are wearing, but an enchantment still turns it. Magic goes around everything. There are no exceptions: an arrow or dart trap fires a missile, so it takes the plus; every other damage source -- a spell, a wand, a blast, a trick shot's burst -- is magic and takes nothing.

Everything a creature wears or holds folds into those four numbers before they are rolled, and combat never learns what supplied them. A long sword, a suit of plate mail and a ring of protection all arrive as the same kind of modifier.


Micro-HP: the pool is not the defence
-------------------------------------

The instinct when making something dangerous is to give it more hit points. In nihilurk that makes it *slower*, not scarier, and the difference matters.

A creature's survival comes from winning the armour roll, not from absorbing many losses. A troll with `armor: 6, armor_bonus: 1` shrugs off most of what a dagger can produce; it does not need thirty hit points to feel dangerous, and if it had them the fight would just take longer while being no more frightening. The whole bestiary lives between 3 and 13 hit points on purpose.

The practical rule: **raise `armor` and `armor_bonus` to make a thing hard to kill; raise `power` and `power_bonus` to make it frightening to stand next to; raise `hp` only to buy a creature one more exchange.**

The tension this buys is Rogue's: every fight is short, every fight can go wrong, and once your armour falls behind, an orc with a lucky roll can end a run that had a plan. Fragility cuts both ways -- the player has twelve hit points.


The two rules that apply only to the player
-------------------------------------------

Both exist to stop the maths above producing a *stalemate*, which is the one outcome a turn-based fight cannot survive.

  * **Excellent hit.** A share of the player's swings (`EXCELLENT_HIT_CHANCE`) roll `EXCELLENT_HIT_DICE` dice of `[power]` instead of one, before armour is subtracted. Without it, a player in poor gear facing good armour has no path at all. It also shatters the suit a foe has on, and a cursed suit throws splinters (`CURSED_SPLINTER_DIE`) into every other foe within `CURSED_SPLINTER_RADIUS`.

  * **Chip damage.** The player's worst swing still takes `CHIP_DAMAGE` off -- but a blow that weak can never be the killing one. It leaves things alive on the last point of HP. Without it, an unlucky player against a well-armoured monster can swing forever and never move the number.

The two dials above and the chip floor (`CHIP_DAMAGE`) are in `constants::combat`. See `../reference/constants.md`.

Monsters get neither. A monster that cannot hurt you simply cannot hurt you, and that asymmetry is what makes armour worth wearing.


The price of a hand
-------------------

One slot holds one thing, and nihilurk has no wait action -- you cannot spend a turn swapping and then act. So the hand you commit is committed until you spend a real turn getting out of it, with whatever is next to you getting a free swing.

That is the whole design of the bow. Drawn, it is the best thing in the dungeon: it looses an arrow on 1d3 where a hand-thrown one rolls 1d2 (a crossbow's quarrel rolls 1d5, a blowgun's venomed dart 1d2), its enchantment rides along on every shot, and a corridor is a killing lane. Swung, it carries `MeleeCap(1)` and is worth a bruise -- less than your bare fists, which is the point. Three hundred swings with a +5 bow deal 300 damage; three hundred with a long sword deal about 2,400; bare-handed, about 1,000.

So an archer is not a melee character with a ranged option. An archer is someone who has decided that nothing will reach them, and has to be right. Letting the bow also be a decent club would collapse that decision into a free upgrade, and the cheapest way to keep a decision honest is to price the thing you did not choose.


Why most gear is cursed
-----------------------

Most weapon, armour and ring drops roll cursed: whatever `NORMAL_QUALITY_PCT` and `EXCEPTIONAL_QUALITY_PCT` leave over. That sounds punishing until you look at the bonus range: a cursed item rolls anywhere in `CURSED_BONUS_MIN..=CURSED_BONUS_MAX`, which reaches above zero, so it is often *better* than the plain ones. What you are gambling is not the number, it is the commitment -- a cursed item cannot be taken off again without a scroll of remove curse, which will destroy it, or the matching scroll of enchantment, which lifts the curse and mends the minus but has to be found and has to match the slot.

A numeric ring is the exception to "better than the plain ones". Its curse rolls from -3 to +2 and the plain ring is +2, so a cursed ring is never the better ring, only the one that will not come off. An exceptional one is a flat +3.

So the drop table is not "most of your loot is bad". It is "most of your loot is a decision". Picking up an unidentified sword and putting it on is the game asking whether you are sure.

The bonus always lands on the flat modifier and never on the die size, so a +3 dagger is still a dagger. Weapon class is a property of the weapon; enchantment is a property of the copy you found.

Traps that scale
----------------

A trap on floor 2 and the same trap on floor 11 are the same row in the same table, but they should not bite the same. The two damage traps -- arrow and dart -- grow with depth in bands that end at the floors in `constants::traps::TRAP_DAMAGE_TIER_LAST_DEPTH`. Each band adds `ARROW_DAMAGE_PER_TIER` to the arrow trap's damage roll and `DART_POWER_DRAIN_PER_TIER` to the dart trap's *permanent* strength drain, so the deepest dart trap that connects costs several points of `power` for the rest of the run.

Those bands are deliberately coarser than the floor-crowding ones (`DIFFICULTY_TIER_LAST_DEPTH`, which steps more often so the deepest floor gets its own worst budget). A trap that stepped that often would out-scale the player; fewer bands are enough to make depth felt.

The bear trap pins your feet and nothing else: you can still swing at whatever walked up to you while you were stuck, and the turns are spent shrinking, not frozen. Trying to *walk* out tears the leg -- a point of damage, a wasted turn, and a floor tile you will recognise later. The dial is `constants::traps::BEAR_TRAP_THRASH_DAMAGE`; the gore is cosmetic and separate.


Reading the existing table
--------------------------

Rough shape of the bestiary, if you want a new creature to sit in it without standing out:

    fodder      hp 3-6    power 4-6     armor 4-6       depth 1
    mid         hp 3-10   power 4-10    armor 4-12      depth 5
    deep        hp 13     power 8-12    armor 6-10      depth 10, with bonuses

Each row also keeps its original Rogue level and armour class in a comment table at the top of `models/src/monsters.rs`, as a design anchor. nihilurk does not model either, but they say what the creature was *for*.


See also
--------

  ../reference/content-tables.md   the fields, the enchantment odds
  ../reference/constants.md        every balance knob and where it lives
  ../how-to/add-a-monster.md       putting a row in
  data-driven-content.md           why a creature is a row at all
  ../../gdd.md                     what the game is trying to be
