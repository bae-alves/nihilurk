How to add a new item category
==============================

    Audience       Engine developer, or a content author who has
                   outgrown the existing categories.
    Prerequisites  You have read `add-an-item.md` and none of its
                   categories fit. You are comfortable with bevy_ecs
                   components and with Rust traits.
    Result         A new kind of item that drops on floors, spawns by
                   name, appears in `-content`, and survives a save.

Do this only when the thing you want is genuinely not one of the existing kinds. A torch is a wand of light with a different flavour; a shield is armour. Reach for a new category when the item carries components no existing `Def` builds, and when it deserves its own share of the drop table.

Worked example below: **food**, a thing you eat for hit points.


Step 1: define the row
----------------------

In `models/src/catalog.rs`, next to the other `Def` structs:

    /// A ration: eaten once, then gone.
    pub struct FoodDef {
        /// The row's identity: the save file stores it, `spawn_named` finds it.
        pub name: &'static str,
        /// The colour it draws in.
        pub color: Color,
        /// What eating it does to the eater.
        pub eat: fn(&mut World, Entity),
    }

A row says what the thing is and names the handler that does its work. Every category is built this way: the row is the potion, the wand, the treat, and the component on the entity points at it.

The component goes in `models/src/components.rs`:

    /// Something to eat. The row says what eating it does.
    #[derive(Component)]
    pub struct Food {
        /// The row it was made from.
        pub row: &'static crate::catalog::FoodDef,
    }

That component is the thing systems will read. `models` warns on any public item without a doc comment, so each field carries one.


Step 2: implement `ItemDef`
---------------------------

    impl ItemDef for &'static FoodDef {
        fn name(&self) -> &'static str {
            self.name
        }

        fn spawn(&self, world: &mut World, pos: Position) -> Entity {
            world
                .spawn((
                    Name { what: strings::content_name(self.name).to_string() },
                    Renderable { glyph: '%', color: self.color },
                    pos,
                    Item,
                    Food { row: self },
                    Consume,
                ))
                .id()
        }
    }

`strings::content_name` translates the row's name for the player; the row keeps its own. `spawn` builds the item *exactly as the row describes it*, with no random rolls. That is what tests, `spawn_named` and `NIHILURK_SPAWN` all want.

If a floor drop should differ from the plain row -- an enchantment, a battery charge, a bundle size, a shuffled deck -- override `spawn_as_loot` as well, and roll it there:

        fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng,
                         pos: Position) -> Entity {
            let e = self.spawn(world, pos);
            world.entity_mut(e).insert(roll_something(rng));
            e
        }

Categories with nothing to roll inherit the default, which just calls `spawn`.

`weight()` and `min_depth()` also have defaults (`DEFAULT_SPAWN_WEIGHT`, and the first floor). Override them only if this category wants per-row rarity -- see `tune-rarity-and-depth.md`.


Step 3: write the table
-----------------------

    /// Every food the dungeon drops, one row each.
    #[rustfmt::skip]
    pub static FOODS: &[&FoodDef] = &[
        // A ration, plain.
        &FoodDef { name: "ration", color: Color::DarkYellow, eat: items::food::ration },
    ];

Every row sits inline in its table. Code that needs one particular row reaches it through an `index_of!` const (`FOODS[RATION]`), never a name of its own.

`#[rustfmt::skip]` keeps your column alignment once the table has more than one row. These tables are read as columns; that is worth more than uniform formatting. The handlers live in a file of their own under `models/src/items/`, the way `potions.rs` holds the potions'.


Step 4: give it a share of the drop table
-----------------------------------------

In `models/src/spawn.rs`, import the table with the others from `crate::catalog` and add one line to `DROPS`:

    category!("food",         120,      1,     FOODS),

That single line wires up everything: floor drops, `spawn_named`, the `-content` listing, and the table tests. The macro erases the table's element type into three function pointers so the whole thing can stay `const`; you never write those by hand. The weights are in `DROPS`.

Every other category's share shrinks in proportion to pay for the new one. No other number needs editing.


Step 5: make it do something
----------------------------

Using an item goes through `item_system` in `models/src/items.rs`. `plan_use` reads the row off each component that holds one (`Potion`, `Wand`, `Scroll`, a `Deck`'s top card, a `Rune`) into a `UsePlan`, and `resolve_use` calls that row's handler. Add yours the same way: a `food` field on `UsePlan`, set from `Food` in `plan_use`, and one line in `resolve_use` that calls `(row.eat)(world, item_use.user)`. `Consume` on the entity already makes the use spend it. The pack's menus pick it up too: `PackMode::Browse` and `PackMode::Use` admit everything.


Step 6: teach the save file
---------------------------

An item is saved as its row's id (`content`) plus a fixed set of component slots in `EntitySave` (`models/src/saveload.rs`). `Food` is not one of them, so by default a saved ration comes back as a nameless husk that does nothing.

Two ways out, cheapest first:

  1. **Rebuild it from the row**, the way a treat comes back. Name the table in `content_id_of` in `catalog.rs`, so a save finds the row's id from the item's name, and extend `restore_from_catalog` beside it:

         if let Some(def) = FOODS.iter().copied().find(|d| d.name == id) {
             entity.insert(Food { row: def });
         }

     The row is the definition, so looking it up by id is always
     cheaper than storing what the row already says. Prefer this.

  2. **Add a field to `EntitySave`.** When the entity carries something the row does not -- a battery's remaining charges, a stack's count, a rune's charge -- or when the category saves its row by name in a field of its own, as potions, scrolls and wands do. Add the field at the end of the struct, read/write it in `save_game` and `load_game`, and bump `SAVE_VERSION`.

> **`EntitySave` field order is the save format.** Postcard encodes
> fields positionally. Append; do not insert or reorder. Any change to a
> saved struct needs a new `SAVE_VERSION`: the test
> `the_save_layout_only_changes_with_the_version` fails until it has one,
> then until you regenerate `models/tests/golden/save.bin` (its message says
> how). The release is then a minor one, and `release/bump.lua` refuses a
> patch.


Step 7: identification, if it is equipment
-------------------------------------------

Only if your category is worn gear whose own quality should be a secret at
first -- a weapon, suit of armour or launcher with an enchantment plus and a
curse it can roll. That means running it through
`crate::catalog::enchant_equipment` at spawn time and letting `KnownQuality`
gate the reveal, same as the three existing equipment categories; copy
whichever is closest.

Every other category -- including potions, scrolls, wands and rings -- is
always shown by its true name. There is no appearance system left to hook
into; a ration is a ration, and so is a potion.


Verify
------

    cargo build
    cargo run -p nihilurk -- -content | grep -A3 '^food'
    NIHILURK_SPAWN="ration" cargo run -p nihilurk
    cargo test

The table tests pick your category up automatically: `every_drop_category_can_actually_produce_something` will fail if you gave it a weight of zero or no rows at its own `min_depth`, and `every_loot_category_can_be_drawn_and_has_rows_to_give` fails the same way, and also if it has no rows at the deepest floor.

Round-trip a save before you call it done -- step 6 is the one people skip, and no existing test reaches a new category. Add one to `models/tests/it/saveload.rs` in the shape of `a_quivers_flight_survives_a_save`: spawn a ration with `spawn_named`, `save_game`, `load_game` into a fresh world, and assert a `Food` came back. Without step 6 it fails.

    cargo test --test it saveload::


Appendix: quick check
---------------------

1. Add the `Def` struct in `models/src/catalog.rs` and the component that holds its row in `models/src/components.rs`.
2. Implement `ItemDef`: `name`, `spawn`, and `spawn_as_loot`, `weight` or `min_depth` only if you need them.
3. Write the table as a `pub static` of `&Def` rows with `#[rustfmt::skip]`, and its handlers under `models/src/items/`.
4. Add one `category!(..)` line to `DROPS` in `models/src/spawn.rs`.
5. Behaviour: read the row in `plan_use` and call its handler in `resolve_use`.
6. Save: name the table in `content_id_of` and extend `restore_from_catalog`, or append a field to `EntitySave` and bump `SAVE_VERSION`; never insert one.
7. Worn gear with a hidden quality: run `enchant_equipment` at spawn and let `KnownQuality` gate it.
8. Run `cargo run -p nihilurk -- -content | grep -A3 '^<category>'` and `NIHILURK_SPAWN="<name>" cargo run -p nihilurk`.
9. Write the save round-trip test, then run `cargo test`.


See also
--------

  add-an-item.md                   the categories that already exist
  tune-rarity-and-depth.md         picking that weight
  ../reference/spawn-api.md        ItemDef, DropCategory, the category! macro
  ../explanation/data-driven-content.md   why rows carry components
