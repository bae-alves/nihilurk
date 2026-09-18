How to spawn a thing
====================

    Audience       Engine developer, or a content author who wants to
                   look at the row they just wrote.
    Prerequisites  Rust and a passing familiarity with bevy_ecs.
                   `../reference/spawn-api.md` lists the signatures;
                   this page is the recipes.
    Result         A named monster, item, trap or the relic, in the
                   world, on the tile you meant.

One function covers nearly every case:

    spawn_named(&mut world, "dragon", Position { x: 10, y: 4 })

It searches every table by name and returns `Option<Entity>`. Four recipes follow -- looking at something now, one in a test, one from game code mid-run, and loot the way the dungeon rolls it. Take the one you are standing in.


Recipe 1: look at it right now
------------------------------

No code. `NIHILURK_SPAWN` drops names on free tiles around the player on every floor it builds:

    cargo run -p engine -- -content              # 1. what are the names?
    cargo run -p engine -- -content | grep -i wand
    NIHILURK_SPAWN="dragon" cargo run -p engine      # 2. put one in front of me
    NIHILURK_SPAWN="bow,arrow,arrow,dart trap" cargo run -p engine

Names are comma-separated and trimmed; a name the tables do not know is skipped **silently**, so check it against `-content` rather than trusting an empty floor. Full rules: `../reference/cli-and-env.md`.


Recipe 2: one specific thing in a test
--------------------------------------

1. **Build a world.** A bare one is enough to spawn into and assert about, and needs no resources at all:

       use bevy_ecs::prelude::*;
       use models::*;

       let mut w = World::new();
       let dragon = spawn_named(&mut w, "dragon", Position { x: 5, y: 5 }).unwrap();

   That is what `models/tests/content.rs` does for all hundred-odd rows.
   Use it whenever the thing itself is the subject -- its components, its
   name, its glyph.

2. **Or build a real floor**, when the thing has to be *somewhere* -- a monster that walks, an item auto-explore has to find, anything that reads `Map`:

       fn test_world(seed: u64) -> World {
           let mut w = World::new();
           w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
           w.insert_resource(RngSeed(seed));
           w.init_resource::<GameLog>();
           w.insert_resource(PlayerName { what: "TESTER".into() });
           initialize_world(&mut w);   // map, player, starting kit, floor 1
           w
       }

   Copy it from `models/tests/identify.rs`, and add the queue your system
   reads (`w.init_resource::<UseQueue>()`, `ThrowQueue`, `AttackQueue`) if
   you are about to run one. The player is
   `w.query_filtered::<Entity, With<Player>>().single(&w)`.

3. **Put it where the test needs it.** Three places a thing can be, and the second and third are alternatives:

       // on the floor -- it already is, from the Position you passed
       let pos = Position { x: 5, y: 5 };
       let sword = spawn_named(&mut w, "long sword", pos).unwrap();

       // in the pack -- strips Position, merges ammo, respects PACK_CAPACITY
       stow(&mut w, player, sword);

       // or worn/wielded -- no log line, no turn, no pack slot needed
       assert!(equip_silently(&mut w, player, sword));

   `initialize_world` hands the player a starting kit -- ring mail, a
   mace, a short bow -- and **that kit is already on**. `Slot::Body` and
   `Slot::Hand` hold one thing each, so `equip_silently` returns `false`
   for a second suit of armour until you take the first one off:

       if let Some(worn) = equipped_in(&w, player, Slot::Body) {
           force_unequip(&mut w, worn);
       }

   If the kit competes with what you are testing in the pack rather than
   on the body, empty the pack first -- `empty_pack` in
   `models/tests/identify.rs` is the pattern.

4. **Name it in a literal and let it panic.** `MonsterDef::named("dragon")` and `TrapDef::of(TrapEffect::Bear)` panic on a miss, which is what you want from a typo in a test. Save `lookup` for names that came from outside the source.


Recipe 3: one from game code, mid-run
-------------------------------------

The scroll of create monster (`models/src/items/scrolls.rs`, `create_monster`) is the worked example. Four steps, in this order:

1. **Find a free tile.** Never assume one:

       let spot = free_adjacent_tile(world, origin)      // next to someone
           .or_else(|| random_open_tile(world));         // anywhere on the floor
       let Some((x, y)) = spot else {
           world.resource_mut::<GameLog>().add("...nothing happens.");
           return;
       };

   Both read the `Map` and both can fail. The refusal branch is not
   optional -- a sealed-in player is a legal floor.

   These two are **inside `models` only** (`crate::helpers` and
   `crate::traps`, neither re-exported). From a test or from `engine/`,
   either pick the tile yourself against `Map::blocks` or call
   `spawn_list`, which does the searching for you.

2. **Roll from `GameRng`, in a scoped block.** The live run rolls from `GameRng` and never from `layout_rng`/`content_rng` -- those two belong to floor generation, and borrowing one while spawning will not compile anyway:

       let idx = {
           let mut rng = world.resource_mut::<GameRng>();
           rng.0.gen_range(0..BESTIARY.len())
       };   // <- borrow ends here, before the spawn

3. **Spawn through the right door:**

       let pos = Position { x, y };
       spawn_monster(world, &BESTIARY[idx], pos);        // monsters
       spawn_named(world, "long sword", pos);            // items, by name
       world.spawn(TrapBundle::from_def(                 // a trap is a bundle,
           TrapDef::of(TrapEffect::Bear),                //   not an entity
           TrapReveal::Sight,
           pos,
       ));

   `spawn_monster` is the only way a monster should enter the world: it
   attaches the row's granted effects and its `Invisible` marker.
   `TrapBundle::random` is the rolled version, but it wants a
   `ChaCha12Rng` of its own -- floor generation hands it a `content_rng`.
   From a live system, name the trap you mean.

4. **Say so in the log.** Use `item_label(world, entity)` (`helpers`, models-internal again) for the name the player is allowed to know -- an unidentified suit of armour is "leather armor", not "+2 leather armor" -- and `article_for(&name)` for the "a"/"an" in front of it.


Recipe 4: loot the way the dungeon rolls it
-------------------------------------------

`spawn_named` gives you the row. `roll_item` gives you a *drop*:

    let item = roll_item(world, &mut rng, depth, pos);

Weighted category, weighted row within it, then `spawn_as_loot` -- so it arrives enchanted, charged and bundled, gated to the depth you passed. This is the only place the dungeon decides what loot exists; call it rather than re-rolling quality yourself.

The `rng` is the caller's, not `GameRng`: floor generation passes its own `content_rng(seed, depth, changes)`, which is what keeps a floor's contents keyed to the seed and the staircase count rather than to the fighting. A test can pass any `ChaCha12Rng::seed_from_u64`.

To roll one piece of quality onto something you spawned by name:

    let sword = spawn_named(world, "long sword", pos).unwrap();
    enchant_equipment(world, &mut rng, sword);    // +1..+3, or cursed

    let wand = spawn_named(world, "wand of fire", pos).unwrap();
    if let Some(mut b) = world.get_mut::<Battery>(wand) {
        b.charges = roll_wand_charges(&mut rng);
    }


What comes out is the bare row
------------------------------

`spawn_named` and the `spawn_*` wrappers build the thing **exactly as its table row describes it** -- no dice are rolled. What that means per kind:

| You spawn | You get | Not |
|-----------|---------|-----|
| a wand    | `Battery { charges: 0 }` -- one zap and it crumbles | a battery rolled off `constants::wands` |
| a weapon, armour, a ring | no bonus, no `Curse` | an enchantment roll |
| arrows, quarrels | `Stack { count: 1 }` | a bundle |
| a trap    | `TrapReveal::Sight`, so you can see what you placed | a rolled reveal style |
| a monster | the row, effects and invisibility included | anything depth-scaled |

That is deliberate: a test that spawns a thing needs it to be the same thing every time. When you want the randomised version, use Recipe 4.


When it does not work
---------------------

**`spawn_named` returned `None`.** The lookup is a case-sensitive exact match on the row's name -- `"Dragon"` and `"long Sword"` find nothing. Check `cargo run -p engine -- -content`.

**It spawned but nothing is drawn.** Either it has no `Position` (being in a `Backpack` means exactly that: `stow` removes it, and the renderer draws the pack separately), or the tile is not currently visible -- floor items are only drawn on revealed tiles, and a monster gets `Hidden` until the player's viewshed reaches it.

**It landed in a wall.** Nothing in the spawn API validates the tile you hand it; `Position` is whatever you said. Use `free_adjacent_tile`, `random_open_tile`, or `spawn_list`, which searches outward in rings and skips anything `Map::blocks`.

**`equip_silently` returned `false`.** Either the item has no `Equipped` component (it is not gear), or the slot is full -- and on a world built by `initialize_world` the body and hand slots start full. `force_unequip` the incumbent first (Recipe 2, step 3).

**The borrow checker refuses the RNG line.** A `resource_mut::<GameRng>()` guard is still alive at the spawn. Roll inside a block and let it drop (Recipe 3, step 2).

**Two things landed on the same tile.** Only `spawn_list` and `spawn_requested` track occupancy, via the `&mut HashSet<(u16, u16)>` you pass in. Spawning in a loop yourself means keeping that set yourself.

**A seeded test started failing after you added content.** Floor layout is a pure function of `(seed, depth)` and cannot move -- but the *contents* are drawn from a weighted table, so adding a row changes which rows a given roll lands on. Assert on what you spawned by name, not on what a seed happened to produce. `models/tests/determinism.rs` holds the line that matters.


Check yourself
--------------

    cargo test --test content        # every name still spawns and keeps its name
    cargo test --test determinism    # seeds still mean what they meant
    cargo run -p engine -- -content  # what the game knows, live from the tables


See also
--------

  ../reference/spawn-api.md       every signature on this page
  ../reference/cli-and-env.md     NIHILURK_SPAWN and -content in full
  ../reference/content-tables.md  the tables these functions read
  add-a-monster.md                adding the row you want to spawn
  add-an-item.md                  the same, for items
  work-with-the-ecs.md            spawning from inside a system
