How to add a new item category
==============================

    Audience       Engine developer, or a content author who has
                   outgrown the nine existing categories.
    Prerequisites  You have read `add-an-item.md` and none of its nine
                   categories fit. You are comfortable with bevy_ecs
                   components and with Rust traits.
    Result         A tenth kind of item that drops on floors, spawns by
                   name, appears in `-content`, and survives a save.

Do this only when the thing you want is genuinely not one of the nine. A torch is a wand of light with a different flavour; a shield is armour. Reach for a new category when the item carries components no existing `Def` builds, and when it deserves its own share of the drop table.

Worked example below: **food**, a thing you eat for hit points.


Step 1: define the row
----------------------

In `models/src/catalog.rs`, next to the other `Def` structs:

    /// A ration: eaten once, then gone.
    pub struct FoodDef {
        pub name: &'static str,
        pub color: Color,
        /// Hit points restored.
        pub nutrition: i32,
    }

A row is a description, never behaviour. Keep it to plain data: what it is called, how it draws, and the numbers its components need.

You will also need the component the row attaches, in `models/src/components.rs`:

    /// Eaten for hit points.
    #[derive(Component)]
    pub struct Food {
        pub nutrition: i32,
    }

That component is the thing systems will read. The row only puts it on.


Step 2: implement `ItemDef`
---------------------------

    impl ItemDef for FoodDef {
        fn name(&self) -> &'static str {
            self.name
        }

        fn spawn(&self, world: &mut World, pos: Position) -> Entity {
            world
                .spawn((
                    Name { what: self.name.to_string() },
                    Renderable { glyph: '%', color: self.color },
                    pos,
                    Item,
                    Food { nutrition: self.nutrition },
                    Consume,
                ))
                .id()
        }
    }

`spawn` builds the item *exactly as the row describes it*, with no random rolls. That is what tests, `spawn_named` and `ROOG_SPAWN` all want.

If a floor drop should differ from the plain row -- an enchantment, a battery charge, a bundle size -- override `spawn_as_loot` as well:

        fn spawn_as_loot(&self, world: &mut World, rng: &mut ChaCha12Rng,
                         pos: Position) -> Entity {
            let e = self.spawn(world, pos);
            // roll whatever this category rolls
            e
        }

Categories with nothing to roll inherit the default, which just calls `spawn`.

`weight()` and `min_depth()` also have defaults (10 and 1). Override them only if this category wants per-row rarity -- see `tune-rarity-and-depth.md`.


Step 3: write the table
-----------------------

    #[rustfmt::skip]
    pub const FOODS: &[FoodDef] = &[
        FoodDef { name: "ration",      color: Color::DarkYellow, nutrition: 8 },
        FoodDef { name: "slime mold",  color: Color::Green,      nutrition: 3 },
    ];

`#[rustfmt::skip]` keeps your column alignment. These tables are read as columns; that is worth more than uniform formatting.


Step 4: give it a share of the drop table
-----------------------------------------

In `models/src/spawn.rs`, add one line to `DROPS`:

    category!("food",         120,      1,     FOODS),

That single line wires up everything: floor drops, `spawn_named`, the `-content` listing, and the table tests. The macro erases the table's element type into three function pointers so the whole thing can stay `const`; you never write those by hand.

Every other category's share shrinks in proportion to pay for the new one. No other number needs editing.


Step 5: make it do something
----------------------------

Everything so far is description. If your item needs behaviour, that lives where behaviour lives -- `item_system` in `models/src/items.rs` dispatches on the component your row attached, handing off to a file under `models/src/items/` (`potions.rs`, `scrolls.rs`, `wands.rs`, `throwing.rs`). Follow how `Potion` is handled: a component the row put on, read by a system that does not know the catalog exists.


Step 6: teach the save file, if it needs teaching
-------------------------------------------------

An item is saved as its name plus a fixed set of component slots in `EntitySave` (`models/src/saveload.rs`). Your new `Food` component is not one of them, so by default a saved ration comes back as a nameless husk that does nothing.

Two ways out, cheapest first:

  1. **Rebuild it from the row.** Extend `restore_from_catalog` in `catalog.rs`, which already does exactly this for weapons, ammo and launchers:

         if let Some(def) = FOODS.iter().find(|d| d.name == name) {
             entity.insert((Food { nutrition: def.nutrition }, Consume));
         }

     The row is the definition, so looking it up by name is always
     cheaper than storing what the row already says. Prefer this.

  2. **Add a field to `EntitySave`.** Only when the value differs per entity -- a battery's remaining charges, a stack's count. Add the field at the end of the struct and read/write it in `save_game` and `load_game`.

> **`EntitySave` field order is the save format.** Postcard encodes
> fields positionally. Append; do not insert or reorder.


Step 7: identification, if it is a mystery item
-----------------------------------------------

Only if your category should arrive unidentified. That is a larger job than the rest of this page put together: a new effect enum, an appearance pool in `models/src/identify.rs`, entries in `ItemAppearances` and `Identified`, and an arm in `display_name`. Four categories do this (potions, scrolls, wands, rings); copy whichever is closest.

Most categories should not. A ration is a ration.


Verify
------

    cargo build
    cargo run -p engine -- -content | grep -A3 '^food'
    ROOG_SPAWN="ration" cargo run -p engine
    cargo test

The table tests pick your category up automatically: `every_drop_category_can_actually_produce_something` will fail if you gave it a weight of zero or no rows at its own `min_depth`, and `the_loot_table_covers_every_category_over_a_long_run` will fail if twenty thousand drops never produce one.

Round-trip a save before you call it done -- step 6 is the one people skip:

    cargo test --test saveload


See also
--------

  add-an-item.md                   the nine categories that already exist
  tune-rarity-and-depth.md         picking that weight
  ../reference/spawn-api.md        ItemDef, DropCategory, the category! macro
  ../explanation/data-driven-content.md   why rows carry components
