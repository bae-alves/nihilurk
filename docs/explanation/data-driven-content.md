Why content in nihilurk is a table
==============================

    Audience       Anyone deciding how to extend nihilurk, or wondering why
                   adding a monster is a one-line change.
    Prerequisites  You have added something, or read
                   `../how-to/add-a-monster.md`.
    This is        Understanding, not instructions. Nothing here tells
                   you what to type.

The short version: **nihilurk is an engine that displays and resolves content, and the content is a set of tables.** The engine has no opinion about what a dragon is. It knows how to draw a glyph, roll two dice against each other, and ask an entity whether it carries a component. The dragon is a row.


The problem this solves
-----------------------

The default shape of a roguelike codebase is a constructor per monster and a `match` per item kind:

    fn spawn_dragon(world, pos) { /* twenty lines */ }
    fn spawn_bat(world, pos) { /* twenty lines */ }

    match ring.effect {
        RingEffect::Protection => armor += 2,
        RingEffect::Strength   => power += 2,
        ...
    }

It works, and it rots in a specific way. Every new creature is twenty lines of near-duplicate. Every new ring is an arm in a match, and there turn out to be seven such matches -- combat, the HUD, saving, loading, the wand of cancellation, the display name, the throw code. Adding content becomes an exercise in remembering all seven places. The compiler catches some of them and not others, and the ones it misses ship as an item that does nothing.

The failure is not verbosity. It is that **the definition of a dragon is smeared across the codebase**, so there is no single place to look, and no way to be sure you have found all of it.


The three moves
---------------

### 1. A row is the definition, and there is only one

    MonsterDef::row("dragon", 'D', Color::Red, Chase, 8, 12, 2, 10, 2, 10)
        .grants(&[Grant::of::<FireImmune>()]),

That line is the entire dragon. Level population reads it, the scroll of create monster reads it, the wand of polymorph reads it, the save file reads it back by name on load. Nothing else in the codebase enumerates species -- so there is no second list to fall out of step, and the question "what is a dragon" has exactly one answer.

The same holds for every category. Nine item tables, one bestiary, one trap table, one loot table.

### 2. Behaviour is a component the row attaches, not code the row names

This is the move that makes the first one possible.

A ring of protection is not a `RingEffect::Protection` that seven files have to recognise. It is an item carrying `ArmorBonus(2)` -- the exact component plate mail carries. Combat already folds every `ArmorBonus` on everything you have equipped, so the ring works before anybody writes a line of ring code. It is not that rings are handled well; it is that **there is no ring code at all**.

A ring of perception is an item that grants `SeesInvisible`. The visibility system already asks "does this entity see invisible things?", because the phantom made it ask. The ring answers a question that was already being asked.

A bow does not know arrows exist. It grants `FireArrow`. An arrow is a thing that says it answers to `FireArrow`. They meet at the effect and nowhere else, which is why a sling is one row in each table and no new code.

So the design rule is: **when you want new behaviour, first look for a question the engine already asks.** Adding a row that answers an existing question is free. Adding a new question is a small, contained change -- one component, one registry entry, one place that reads it (see `../how-to/add-an-effect.md`).

### 3. Rarity is data on the row, not arithmetic in the spawner

The loot table used to be this:

    match rng.gen_range(0..100) {
        0..=29 => one(world, rng, pos, SCROLLS),
        30..=56 => one(world, rng, pos, POTIONS),
        57..=73 => one(world, rng, pos, COINS),
        ...
    }

Adding a category meant recomputing every boundary by hand and hoping the last arm still reached 99. The percentages were a constraint the author had to satisfy, and they were satisfied nowhere in particular.

It is now a weighted table:

    category!("scroll",  300,  1,  SCROLLS),
    category!("potion",  270,  1,  POTIONS),

A weight has no meaning on its own, only against its table-mates, so adding a row cannot invalidate another row. The same two dials -- `weight` and `min_depth` -- appear on monsters and traps, so "how often" and "how deep" are asked and answered the same way everywhere.


Where the pieces live
---------------------

Every kind of game object -- monster, item, trap -- is built the same three-part way, and each part has one home:

  * **The components** (`Trap`, `Fighter`, `Snare`, `FireArrow`, ...) go in `models/src/components.rs`. That file is nouns: every `Component`, `Resource` and `Event` the game is made of, and nothing about what happens as a result. A system reads them; the save file round-trips them.

  * **The table and its `Def` row** (`BESTIARY` / `MonsterDef`, the nine item tables, `TRAPS` / `TrapDef`) go in that object type's own module -- `monsters.rs`, `catalog.rs`, `traps.rs`. A row is plain data: a name, a glyph, and the numbers its components need. Row-level dials (`weight`, `min_depth`, a trap's `snare_turns`) live here too, so adding a row that wants its own value is still a one-file change -- not a trip to `constants.rs`.

  * **The `Bundle`** -- the struct that assembles the components for one spawn (`MonsterBundle`, `TrapBundle`) -- lives *next to its table*, not in `components.rs`. It is glue between a `Def` and the ECS, it names the row type, and it is the one place an entity of that kind is described. Keeping it with the table keeps "how a trap is made" in one file.

So a brand-new object type -- say hazards that are neither trap nor monster -- is a new module with its table, `Def` and `Bundle`, plus whatever new components it needs in `components.rs`, plus the system that drives it. `constants.rs` only enters if the type has a game-wide tuning knob, and even then the per-row numbers stay on the row.


The seed contract
-----------------

Content being data raises a question that content-as-code does not: if the dungeon is generated by drawing on a random number stream, does adding a row move everybody's dungeon?

It must not, and it cannot. **A floor's *layout* is a pure function of `(seed, depth)`.** Its *contents* are looser -- keyed to the seed, the depth, and the staircase count -- but never to the shared `GameRng`. Each floor gets two private streams:

    layout_rng(seed, depth)             rooms, corridors, doors, stairs
    content_rng(seed, depth, changes)   monsters, loot, traps, placement

`changes` is `FloorChanges` -- how many times this run the player has taken a staircase, portal or trapdoor. `GameRng` is the live stream the *run* spends -- combat rolls, item effects, traps going off -- and nothing that draws on it can reach a floor's generation.

That buys:

  * **Adding content never rearranges the dungeon.** Add a monster, triple a drop weight, add a whole category -- the walls do not move. Seeds people wrote down keep meaning what they meant. (What the new row can do, of course, is turn up: a bigger table has more in it.)

  * **A seed names one set of maps, however you play it.** Fight everything on floor 1 or run straight past it, and floor 2 is the same place. Two runs that descend in lockstep meet the same monsters and loot too; what breaks the tie is only the staircase count, never the blow-by-blow.

  * **A save does not store the map.** It stores a seed, a depth and the staircase count, and the loader rebuilds the exact floor -- layout and contents. Reloading mid-run cannot shift the next floor.

  * **Climbing back up returns you to the same maze, freshly stocked.** The back half of a run -- carrying the Element of Yoord out -- is a walk through corridors you recognise, but the count has moved on, so `content_rng` re-rolls: different monsters, different loot, and the fog of war is blank again (per-floor memory is not kept, to keep the save small). Same place, new problem.

The same seed-salt trick keys item appearances off a third stream, so adding to an appearance pool cannot perturb the dungeon either.

Two players on one seed diverge the moment their staircase counts do. The maps are fixed; the run is not.

`models/tests/determinism.rs` holds the contract to the fire -- layout is identical across reloads and however much of the shared stream a run burns; contents match between lockstep descents and *differ* on a repeat visit. If the layout half ever fails, floor generation has started reading something it should not.

What follows from it
--------------------

**Tools come free.** Because content is data, anything that walks the tables works on all of it at once, forever:

    cargo run -p engine -- -content      lists every name, live
    NIHILURK_SPAWN="dragon,bow"              spawns any of them
    spawn_named(world, name, pos)        one door for tests and scripts
    models/tests/content.rs              tests the data as data

None of those has a list of content in it. They read the tables. A row added today is covered by all four today.

**Tests get stronger, not longer.** `every_content_name_spawns_and_keeps_its_name` iterates the tables. Adding a monster adds a test case. This is the practical difference between content-as-code and content-as-data: you can write assertions that quantify over all content.

**Saves get smaller and more robust.** A saved item stores its *name*, not its components; `restore_from_catalog` rebuilds the rest from the row. A save that stored a bow's grant list would only be storing the table twice, and would go stale the moment the table changed.

**The gaps become visible, and closing one is a chain.** Seven of the twelve rings used to have a name and an appearance and no content. In the old shape that would have been seven missing match arms scattered about, indistinguishable from bugs; here it was seven rows with no chains -- unfinished in a way you could see at a glance. Six of the seven were finished by adding exactly that: `.power_bonus(2)` for increase damage, `.grants(&[Grant::of::<Stealthy>()])` for stealth, and so on for regeneration, slow digestion, teleportation and maintain armor. Two of those needed a new marker in `EFFECTS` and a system that reads it; none of them needed `catalog.rs` to learn what a ring of stealth is.


Where it does not reach
-----------------------

Honesty about the seams, because they are where people get stuck.

**Potions, scrolls and wands still need a mechanic.** "Restore hit points" is not expressible as a component the engine already folds, so those three categories keep an effect enum and a `match` (in `models/src/items/potions.rs`, `scrolls.rs` and `wands.rs` respectively). Those matches are exhaustive now, the same as traps: no catch-all arm, so a variant given no arm does not build, rather than compiling and shipping as a silent dud.

That buys the missing-arm case, not the missing-*behaviour* case -- exhaustiveness only proves every variant was mentioned, not that what it does is finished. `PotionEffect` is fully wired now (its two do-nothing arms, `FruitJuice` and `Water`, are deliberate: they are a taste and a log line, and the fact that they report *no* visible effect is what keeps a thrown one from naming itself). `ScrollEffect` is fully wired too, as of the six that used to share an explicit do-nothing arm (`MonsterConfusion`, `HoldMonster`, `Sleep`, `EnchantArmor`, `FoodDetection`, `EnchantWeapon`); `BlankPaper` is the one arm left that does nothing, and it is the joke, not a gap. What exhaustiveness buys is that nobody can add a *new* such gap by accident -- a new variant has to be named in the match, whether the arm you give it is real behaviour or an honest placeholder.

**And one ring did need a verb.** Eleven of the twelve are a number or a marker; the ring of adornment is an *event* -- it fires once, when it goes on, and spends itself doing it. A `Grant` cannot say that, so it rides as an `OnWear` component the row attaches, and `models/src/items/rings.rs` holds the three ring verbs that exist (the adornment flourish, the regeneration tick, the teleportitis jump). That file is the honest cost of the design: it is where a ring's behaviour goes when the row cannot hold it. It still contains no `match` on `RingEffect`, and nothing outside it knows which ring is which.

Wiring the potions is also where the third home for a mechanic showed up. A condition (confusion, blindness, paralysis, a shifted tempo) is not a potion's property any more than it is a wand's: both put the same affliction on the same creature, and both have to know that the player takes it as a marker component the input loop reads while a monster takes it as a `MovementType` or a slower tempo. So the verbs live in `models/src/conditions.rs`, one per affliction, and the potion arm and the wand arm are each one line into them.

**Numeric modifiers need a place to be *read*.** A new `SightBonus` is one row in `modifiers!`, and that row folds it into `Loadout` for you -- but somebody still has to read the field from the calculation it modifies. There is no generic answer to "where does a new number belong".

**Floor budgets are still code.** How many monsters and traps a floor gets, and how that scales with depth, lives in `populate_level`. It is the same for every row, so it is not content -- but it is a hand-tuned formula, and it is the thing to change when the dungeon feels wrong.

**Content is compiled in.** Adding a row means a rebuild. That is a deliberate trade; see `adr-0001-tables-not-raws.md`.


The test that keeps this honest
-------------------------------

If you want one sentence that captures the design, it is the assertion in `models/tests/content.rs`:

    for (category, name) in content_names() {
        let entity = spawn_named(&mut w, name, at(5, 5)).unwrap();
        assert_eq!(&w.get::<Name>(entity).unwrap().what, name);
    }

Every piece of content in the game, enumerated, built, and checked, by code that names none of it.


See also
--------

  adr-0001-tables-not-raws.md      why not JSON files
  ecs-in-nihilurk.md                   the ECS half of the same argument
  ../reference/content-tables.md   the tables themselves
  ../how-to/add-an-effect.md       adding a new question the engine asks
