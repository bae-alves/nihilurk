Why monsters are so fragile
===========================

    Audience       Anyone choosing numbers for a new monster or a new
                   piece of gear and wondering what "good" looks like.
    Prerequisites  None. Helps to have read a bestiary row.
    This is        Understanding, not instructions. For the numbers
                   themselves see `../reference/content-tables.md`; for
                   the act of adding a row, `../how-to/add-a-monster.md`.

A goblin has one hit point. That is not a placeholder.


One exchange, two dice
----------------------

roog resolves a fight as two independent rolls, subtracted:

    damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)

Both sides roll every time. There is no to-hit roll and nothing ever
misses -- a swing that fails is not a miss, it is a hit the armour ate.

Everything a creature wears or holds folds into those four numbers before
they are rolled, and combat never learns what supplied them. A long
sword, a suit of plate mail and a ring of protection all arrive as the
same kind of modifier.


Micro-HP: the pool is not the defence
-------------------------------------

The instinct when making something dangerous is to give it more hit
points. In roog that makes it *slower*, not scarier, and the difference
matters.

A creature's survival comes from winning the armour roll, not from
absorbing many losses. A troll with `armor: 6, armor_bonus: 1` shrugs off
most of what a dagger can produce; it does not need thirty hit points to
feel dangerous, and if it had them the fight would just take longer while
being no more frightening. The whole bestiary lives between 1 and 12 hit
points on purpose.

The practical rule: **raise `armor` and `armor_bonus` to make a thing
hard to kill; raise `power` and `power_bonus` to make it frightening to
stand next to; raise `hp` only to buy a creature one more exchange.**

The tension this buys is Rogue's: every fight is short, every fight can
go wrong, and a goblin with a lucky roll can end a run that had a plan.
Fragility cuts both ways -- the player has twelve hit points.


The two rules that apply only to the player
-------------------------------------------

Both exist to stop the maths above producing a *stalemate*, which is the
one outcome a turn-based fight cannot survive.

  * **Excellent hit.** 15% of the player's swings roll `3d[power]`
    instead of `1d[power]`, before armour is subtracted. Without it, a
    player in poor gear facing good armour has no path at all.

  * **Chip damage.** The player's worst swing still takes one point off
    -- but a blow that weak can never be the killing one. It leaves
    things alive on 1 HP. Without it, an unlucky player against a
    well-armoured monster can swing forever and never move the number.

Monsters get neither. A monster that cannot hurt you simply cannot hurt
you, and that asymmetry is what makes armour worth wearing.


The price of a hand
-------------------

One slot holds one thing, and roog has no wait action -- you cannot spend
a turn swapping and then act. So the hand you commit is committed until
you spend a real turn getting out of it, with whatever is next to you
getting a free swing.

That is the whole design of the bow. Drawn, it is the best thing in the
dungeon: it doubles an arrow's die, its enchantment rides along on every
shot, and a corridor is a killing lane. Swung, it carries `MeleeCap(1)`
and is worth a bruise -- less than your bare fists, which is the point.
Three hundred swings with a +5 bow deal 300 damage; three hundred with a
long sword deal about 2,400; bare-handed, about 1,000.

So an archer is not a melee character with a ranged option. An archer is
someone who has decided that nothing will reach them, and has to be right.
Letting the bow also be a decent club would collapse that decision into a
free upgrade, and the cheapest way to keep a decision honest is to price
the thing you did not choose.


Why most gear is cursed
-----------------------

Two thirds of weapon, armour and ring drops roll cursed. That sounds
punishing until you look at the bonus range: a cursed item rolls between
-5 and +5, so it is often *better* than the 25% that roll plain. What
you are gambling is not the number, it is the commitment -- a cursed item
cannot be taken off again without a scroll of remove curse, which will
destroy it.

So the drop table is not "most of your loot is bad". It is "most of your
loot is a decision". Picking up an unidentified sword and putting it on
is the game asking whether you are sure.

The bonus always lands on the flat modifier and never on the die size, so
a +3 dagger is still a dagger. Weapon class is a property of the weapon;
enchantment is a property of the copy you found.


Reading the existing table
--------------------------

Rough shape of the bestiary, if you want a new creature to sit in it
without standing out:

    fodder      hp 1-2    power 4-8     armor 4-8       depth 1
    mid         hp 3-6    power 6-10    armor 6-10      depth 3-5
    deep        hp 8-12   power 8-12    armor 6-10      depth 7, with bonuses

Each row also keeps its original Rogue level and armour class in a
comment table at the top of `models/src/monsters.rs`, as a design anchor.
roog does not model either, but they say what the creature was *for*.


See also
--------

  ../reference/content-tables.md   the fields, the enchantment odds
  ../how-to/add-a-monster.md       putting a row in
  data-driven-content.md           why a creature is a row at all
  ../../gdd.md                     what the game is trying to be
