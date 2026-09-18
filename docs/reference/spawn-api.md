Reference: the spawn API
========================

    Audience       Engine developer, or anyone writing a test that
                   needs a specific thing on a specific tile.
    Prerequisites  Rust, and a passing familiarity with bevy_ecs.
    Status         Signatures as they are in the source. If this page
                   and the source disagree, the source is right and this
                   page is a bug.

Everything here is re-exported from the crate root, so `use models::*;` is enough.


The one you probably want
-------------------------

    pub fn spawn_named(
        world: &mut World,
        name: &str,
        pos: Position,
    ) -> Option<Entity>

    models/src/spawn.rs

Spawns whatever the game knows by that name -- monster, any item category, trap, or the relic -- and returns `None` if it knows nothing by it. What comes out is the thing **exactly as its row describes it**: no enchantment roll, no battery charge, no ammunition bundle.

    let dragon = spawn_named(&mut world, "dragon", pos).unwrap();
    let sword  = spawn_named(&mut world, "long sword", pos).unwrap();
    assert!(spawn_named(&mut world, "sandwich", pos).is_none());

Lookup is a case-sensitive exact match on the row's name, and the tables are searched in this order: bestiary, then each `DROPS` category in table order, then traps, then the relic. Names are unique across all of them -- `every_content_name_is_unique` enforces it -- so the order does not affect the result.

Roughly a hundred rows, all `const`, all in cache: a linear scan beats building a `HashMap` at startup and is one less thing to keep in sync.


Listing what exists
-------------------

    pub fn content_names() -> Vec<(&'static str, &'static str)>

Every name `spawn_named` answers to, as `(category, name)`, in table order. Categories are `"monster"`, the nine `DROPS` names, `"trap"` and `"relic"`.

Backs the `-content` flag. Reads the tables, so it can never go stale.


Rolling loot
------------

    pub fn roll_item(
        world: &mut World,
        rng: &mut ChaCha12Rng,
        depth: u8,
        pos: Position,
    ) -> Entity

One floor drop: a weighted draw for the category, then a weighted draw within it, then `ItemDef::spawn_as_loot` -- so the result *is* enchanted, *is* charged, *does* arrive as a bundle.

This is the only place the dungeon decides what loot exists.

    pub fn pick_weighted(weights: &[u32], rng: &mut ChaCha12Rng)
        -> Option<usize>

Picks an index in proportion to the weights. `None` for an empty slice or all-zero weights. One RNG draw whatever the table's length.

    pub const DROPS: &[DropCategory]
    impl DropCategory {
        pub fn rows(&self, depth: u8) -> Vec<&'static str>
    }

The loot table, and the rows one category can produce at a depth.


Monsters
--------

    models/src/monsters.rs

    pub fn spawn_monster(
        world: &mut World,
        def: &MonsterDef,
        pos: Position,
    ) -> Entity

The one place a monster is brought into the world. Builds the full bundle, attaches the row's granted effects, and adds `Invisible` if the row asked. Every spawn site goes through it.

    impl MonsterDef {
        pub fn named(name: &str) -> &'static MonsterDef      // panics
        pub fn lookup(name: &str) -> Option<&'static MonsterDef>
        pub fn pick(depth: u8, rng: &mut ChaCha12Rng) -> &'static MonsterDef
        pub fn pick_any(rng: &mut ChaCha12Rng) -> &'static MonsterDef
    }

`named` is for string literals you wrote yourself -- it panics on a miss, which is what you want in a test. `lookup` is for a name from outside the source, such as a save file. `pick` is the weighted, depth-gated draw floor generation uses. `pick_any` is the same weighted draw with the depth gate removed -- floor population switches to it once the player carries the Element of Yoord (`map::levels::holding_element_of_yoord`), so the climb out can throw the whole bestiary at any floor.


Traps
-----

    models/src/traps.rs

    impl TrapDef {
        pub fn of(effect: TrapEffect) -> &'static TrapDef     // panics
        pub fn lookup(name: &str) -> Option<&'static TrapDef>
        pub fn pick(depth: u8, rng: &mut ChaCha12Rng) -> &'static TrapDef
    }

    impl TrapBundle {
        pub fn from_def(def: &TrapDef, reveal: TrapReveal, position: Position)
            -> Self
        pub fn random(rng: &mut ChaCha12Rng, depth: u8, position: Position)
            -> Self
        // plus one named constructor per trap, for tests:
        pub fn trapdoor(position: Position) -> Self
        pub fn bear(position: Position) -> Self
        pub fn sleep(position: Position) -> Self
        pub fn teleport(position: Position) -> Self
        pub fn arrow(position: Position) -> Self
        pub fn dart(position: Position) -> Self
    }

The named constructors all use `TrapReveal::Sight`, so a test can see the trap it placed. `random` rolls both the kind (weighted, depth-gated) and the reveal style (uniform).

`TrapBundle` is a bundle, not an entity: `world.spawn(bundle)`.


Items, by kind
--------------

    models/src/catalog.rs

Convenience wrappers over the tables. Each panics on an unknown name or effect -- they exist for call sites holding a literal.

    pub fn spawn_potion(world, effect: PotionEffect, pos)  -> Entity
    pub fn spawn_scroll(world, effect: ScrollEffect, pos)  -> Entity
    pub fn spawn_wand(world, effect: WandEffect, pos)      -> Entity
    pub fn spawn_ring(world, effect: RingEffect, pos)      -> Entity
    pub fn spawn_weapon(world, name: &str, pos)            -> Entity
    pub fn spawn_armor(world, name: &str, pos)             -> Entity
    pub fn spawn_ammo(world, name: &str, pos)              -> Entity
    pub fn spawn_launcher(world, name: &str, pos)          -> Entity
    pub fn spawn_element_of_yoord(world, pos)              -> Entity

`spawn_named` covers all of these. Prefer it in new code unless you have an effect enum rather than a name.


The `ItemDef` trait
-------------------

    pub trait ItemDef {
        fn name(&self) -> &'static str;
        fn spawn(&self, world: &mut World, pos: Position) -> Entity;
        fn weight(&self) -> u32 { 10 }
        fn min_depth(&self) -> u8 { 1 }
        fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng,
                         pos: Position) -> Entity { self.spawn(world, pos) }
    }

Implemented by all nine item row types so the loot roller can treat every category alike. See `../how-to/add-an-item-category.md`.


Other catalog helpers
---------------------

    pub fn enchant_equipment(world, rng: &mut ChaCha12Rng, item: Entity)

Rolls quality for a freshly spawned piece of gear and stamps the result on. Reads the item to decide which bonus applies.

    pub fn roll_wand_charges(rng: &mut ChaCha12Rng) -> i8

`CHARGE_DICE d CHARGE_SIDES + CHARGE_BONUS`, all three from `constants::wands`. A wand spawned any other way carries `Battery { charges: 0 }`.

    pub fn split_one(world: &mut World, item: Entity) -> Option<Entity>

A fresh single unit of whatever `item` is a stack of, spawned nowhere in particular -- the one arrow that leaves a quiver when you shoot it. `None` if the item is not something the catalog can make more of.

    pub fn restore_from_catalog(entity: &mut EntityWorldMut, name: &str)

Re-attaches what a row gives an item that the save file does not store: how a weapon behaves in flight, what a bow lends its wielder, what a missile answers to. Keyed by name, because the row is the definition.


Effects
-------

    models/src/effects.rs

    impl Grant {
        pub const fn of<C: Component + Default>() -> Self
        pub fn attach(&self, entity: &mut EntityWorldMut)
        pub fn detach(&self, entity: &mut EntityWorldMut)
        pub fn probe(&self, world: &World, entity: Entity) -> bool
    }

A const handle to one marker effect. Lets a `const` table name a component it cannot store.

    pub fn grant_all(world, entity: Entity, grants: &'static [Grant])
    pub fn revoke_all(world, entity: Entity)
    pub fn effects_of(world: &World, entity: Entity) -> EffectSet
    pub fn attach_effects(entity: &mut EntityWorldMut, set: EffectSet)
    pub fn effect_set(grants: &[Grant]) -> EffectSet
    pub fn equipped_total<C: Modifier>(world: &World, entity: Entity) -> i32
    pub fn loadout(world: &World, entity: Entity) -> Loadout

`equipped_total` is where gear turns into *one* number: it sums a modifier across everything an entity has equipped plus anything it carries itself. The throw code folds `ThrowBonus` through it; the trap damage rule folds `ArmorBonus`. Neither knows what kind of item supplied the value.

`loadout` is the same fold with the loop on the outside, and it is what callers that want *all* of them use -- combat, for both sides of a blow, and the HUD, for the five figures on the status line. It returns a `Loadout` (`power_die`, `power_bonus`, `armor_die`, `armor_bonus`, `throw_bonus`, and the strictest `melee_cap`, which folds with `min` rather than `+`). There is no cache and no second copy of anything: `Equipped` is still the only record of who wears what, and this reads it once instead of six times.

The struct is not written out by hand. Every field but the cap is generated from the `modifiers!` rows in `effects.rs`, along with the line of `Loadout::absorb` that sums it, so a modifier cannot be declared and then quietly left out of the fold -- see `../how-to/add-an-effect.md`. The cap is written out separately because it folds with `min`.


The dev shortcut
----------------

    pub fn spawn_requested(
        world: &mut World,
        near: Position,
        occupied: &mut HashSet<(u16, u16)>,
    )

Reads `NIHILURK_SPAWN` and drops each named thing on a free tile around `near`. Called at the end of floor generation.

    pub fn spawn_list(
        world: &mut World,
        list: &str,
        near: Position,
        occupied: &mut HashSet<(u16, u16)>,
    ) -> usize

The same thing without the environment lookup, for tests. Returns how many names the tables recognised.


Floors and seeds
----------------

    models/src/map/

    pub fn layout_rng(seed: u64, depth: u8) -> ChaCha12Rng
    pub fn content_rng(seed: u64, depth: u8, changes: u32) -> ChaCha12Rng
    pub fn difficulty_tier(depth: u8) -> u32

A floor's two private streams: `layout_rng` builds its walls, `content_rng` fills it. `layout_rng` is a pure function of `(seed, depth)`, so the walls of a floor never move. `content_rng` also takes `changes` -- the run's `FloorChanges` count, bumped by every staircase, portal and trapdoor -- so the contents are re-rolled each time the floor is entered. The two streams are independent, so a change to one cannot move the other.

`difficulty_tier(depth)` is the 0-4 crowding band the monster and trap budgets read (`DIFFICULTY_TIER_LAST_DEPTH`).

Floor generation draws from these and never from the shared `GameRng`. That is what keeps a seed's *maps* fixed regardless of how the player fought their way through -- and keeps the contents keyed to a clean staircase count rather than to the blow-by-blow. `GameRng` is for the live run -- combat, item effects, traps springing.

    pub fn create_map(world: &mut World) -> ((u16, u16), Vec<Rect>)

Builds the current floor (reading `Depth` and `RngSeed`) into the `Map` resource and returns the player's start tile. Leaves `GameRng` untouched.

    pub fn regenerate_map(world: &mut World, seed: u64, depth: u8)

Rebuilds one floor's tiles without spawning anything. This is why the save file does not store a map: `(seed, depth)` is enough to get the exact layout back (the save carries the staircase count so the contents come back too).

Held by `models/tests/determinism.rs`.


Constants
---------

    pub const ELEMENT_OF_YOORD: &str = "The Element of Yoord";


See also
--------

  content-tables.md             the tables these functions read
  cli-and-env.md                NIHILURK_SPAWN and the -content flag
  ../how-to/spawn-a-thing.md    recipes for the functions above
  ../how-to/work-with-the-ecs.md      spawning, despawning, and the borrows
  ../how-to/add-an-item-category.md   implementing ItemDef yourself
