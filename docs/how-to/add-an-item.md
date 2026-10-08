How to add an item
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-item.md` first.
    Result         A new item that drops on floors and survives a save.
                   Equipment (weapons, armour, launchers) hides its
                   enchantment plus and curse until worn or identified;
                   every other kind is always shown by its true name.

Twelve categories. Six of them are a single row; the deck has one row and twenty cards. Four also need a name to be identified by, and three of those also need somebody to say what the thing *does*. Find your category below and follow that recipe only.


At a glance
-----------

| Category  | Table       | Row alone? | Also needs                       |
|-----------|-------------|------------|----------------------------------|
| Weapon    | `WEAPONS`   | yes        | --                               |
| Armour    | `ARMORS`    | yes        | --                               |
| Coin      | `COINS`     | yes        | --                               |
| Ammo      | `AMMO`      | yes*       | *a launcher effect to answer to  |
| Launcher  | `LAUNCHERS` | yes*       | *an effect to grant              |
| Treat     | `TREATS`    | yes        | --                               |
| Ring      | `RINGS`     | no         | a `RingEffect` variant           |
| Potion    | `POTIONS`   | no         | a `PotionEffect` variant + a mechanic |
| Scroll    | `SCROLLS`   | no         | a `ScrollEffect` variant + a mechanic |
| Wand      | `WANDS`     | no         | a `WandEffect` variant + a mechanic |
| Deck      | `DECKS`     | yes        | a new card: a `CardFace` variant, a `CARDS` row + an arm in `items/decks.rs` |
| Rune      | `RUNES`     | no         | a `RuneEffect` variant + an arm in `apply_rune_effect` |

Every table is in `models/src/catalog.rs`. Every effect enum is in `models/src/components.rs`. Every mechanic is in `models/src/items/`, one file per kind: `potions.rs`, `scrolls.rs`, `wands.rs`, `throwing.rs` (`models/src/items.rs` itself is just the `item_system` dispatcher).


Row-only categories
-------------------

### Weapon

    WeaponDef::new("war hammer", Color::Grey, 9),

`new(name, colour, power_die)`. The die is what the weapon is worth in a swing: damage rolls `1d[power_die]`. Draws as `)`, worn in the hand slot.

Chain `.missile(die)` if it is built to be thrown -- it then rolls that die on impact, goes around the target's armour die, is spent on what it hits, and can never be caught out of the air. Chain `.piercing()` if a throw should run the whole line instead of stopping at the first body.

    WeaponDef::new("javelin", Color::DarkYellow, 5).missile(7).piercing(),

Chain `.returning()` if the throw should strike the first creature and fly home: into the pack, and back into the hand if the weapon was wielded. Leave off `.missile()` -- a projectile is spent on what it hits.

    WeaponDef::new("boomerang", Color::DarkYellow, 6).returning(),

Chain `.hits(n)` after `.returning()` and one throw strikes `n` creatures in all, each for full damage: the first in its way, then the nearest other enemy -- one the player can see, or for a monster thrower, any in throwing range with a clear line. The moon blade's 3: hit, hit, hit, home.

    WeaponDef::new("moon blade", Color::Cyan, 3).returning().hits(3),

Any weapon can be thrown. `.missile()` is the difference between a purpose-built missile and a hurled lump of metal.

Chain `.reach(n)` for a weapon aimed with its own reticle (`v`) instead of a walk into the target's tile -- a bardiche's 2, a whip's 5. Add `.reach_piercing()` if the strike should run the whole line rather than stopping at the first body, the melee twin of `.piercing()`.

Chain `.grants(&[Grant::of::<SomeMarker>()])` for a trick the weapon lends its wielder while it's in hand -- exactly a ring's own `.grants()`, and read by the same places a ring's effects are: an `ABILITIES` row keyed on `Moment::OnHit` for something that fires when a blow lands, or a direct `world.get::<SomeMarker>(attacker)` probe for something checked elsewhere (the battle axe's `Cleaves`, checked once from `models::melee_attack`). This is also how a weapon-only marker stays invisible to `combat.rs` and `engine/` alike -- see `crate::effects`'s module doc.

Chain `.on_wear(OnWear(some_fn))` for a one-shot fired the instant it's wielded -- the staff's "You're a wizard now!" -- the same mechanism a ring of adornment's flourish uses. `.on_doff(OnDoff(some_fn))` is its mirror, fired when the weapon is deliberately put away again ("You're no longer that magical."), and it is worth adding only when what the weapon lends is invisible on the HUD: the player has no other way to notice it leaving. It fires from `equipment::toggle_equipped` alone, never from `force_unequip`, so dropping it, being disarmed of it and dying with it say nothing.

    WeaponDef::new("bardiche", Color::Grey, 7).reach(2).reach_piercing(),
    WeaponDef::new("garrote", Color::DarkGrey, 0).grants(&[Grant::of::<VorpalOnCondition>()]),

### Armour

    ArmorDef { name: "brigandine", color: Color::Grey, armor_die: 6, grants: &[] },

`armor_die` is what wearing it adds to the defence roll: `1d[armor_die]`. Draws as `]`, worn on the body. The existing nine run 2 (leather) to 9 (plate).

`grants` lends marker effects while it is worn, the same as a weapon's `.grants()`. Spikemail's `&[Grant::of::<Spiked>()]` is read by an `ABILITIES` row at `Moment::OnStruck`, the moment a blow lands on the bearer.

### Coin

    CoinDef { name: "electrum coin", color: Color::DarkYellow,
              effect: PickupEffect::Coin, amount: 2500, weight: 10 },

Coins are the pickup category: never carried, spent where they lie. A row is `name`, `color`, a `PickupEffect`, and one `amount` the effect reads (points, hit points, afflictions lifted). Adding a *kind* of coin means a `PickupEffect` variant and an arm in `items/pickups.rs`; adding another coin of an existing kind is one row. Draws as `$`.

### Ammunition

    AmmoDef { name: "bolt", color: Color::Grey, die: 5,
              launched_by: Grant::of::<FireQuarrel>() },

`die` is what one rolls hurled by hand; a wielder carrying the `launched_by` effect doubles it. Stacks up to `constants::items::STACK_LIMIT` per pack slot, and arrives from the dungeon floor in bundles of `constants::loot::AMMO_BUNDLE_MIN..=AMMO_BUNDLE_MAX`.

If your ammunition answers to a launcher that does not exist yet, you need a new effect for the pair to meet at -- see `add-an-effect.md`.

### Launcher

    LauncherDef { name: "sling", color: Color::DarkYellow,
                  grants: &[Grant::of::<FireStone>()], melee_cap: 1 },

A launcher has no attack die and no armour die. All it does is put an effect on whoever holds it; ammunition that names the same effect is loosed rather than lobbed. Draws as `}`.

`melee_cap` is the most it is worth swung at something, whatever the dice or the enchantment say -- 1 for both existing launchers. It is the price of the hand: a launcher fills the slot a sword would have, and nihilurk has no wait action to swap back with.

Neither half knows the other exists. That is why a sling is one row here and one row in `AMMO`.

### Treat

    TreatDef { name: "bone", color: Color::White, for_item_users: false },

A treat is thrown at a monster as an offer of loyalty and is never used. `for_item_users` says which monsters it is for: those with `ItemUser` (`true`) or those without (`false`). Stacks like ammo. Draws as `%`. What accepting one means is `models/src/companion.rs`.


Rings
-----

A ring is a modifier item, exactly like a sword. Most of them have no behaviour code anywhere -- they stack numbers through the same components combat already folds, and lend marker effects through `Grants`.

1. Append a variant to `RingEffect` in `models/src/components.rs`:

       pub enum RingEffect {
           ...
           MaintainArmor,
           FireResistance,      // <- at the end. See the warning below.
       }

2. Add the row in `RINGS`:

       RingDef::new(RingEffect::FireResistance, "ring of fire resistance")
           .grants(&[Grant::of::<FireImmune>()]),

Available chains:

    .power_bonus(n)     flat modifier on the wearer's damage roll
    .armor_bonus(n)     flat modifier on the wearer's armour roll
    .throw_bonus(n)     flat modifier on everything they throw
    .grants(&[...])     marker effects lent while worn
    .on_wear(...)       a one-shot fired the instant it goes on

A bonus of zero attaches nothing, so an inert ring costs nothing at run time. Giving a ring a body is adding a chain to its existing row.

Reach for `.grants(...)` before anything else: if the property already exists as a marker some system asks about, you are done. If it does not, add the marker (`../how-to/add-an-effect.md`) and the one system that reads it -- that is how stealth, regeneration, teleportitis and maintain armor were wired, and none of them put a line in `catalog.rs` beyond the row.

`.on_wear(...)` is the last resort, for a ring whose effect is an *event* rather than a property: it takes an `OnWear(fn(&mut World, wearer, item))` and fires once, after the ring is worn and identified. The ring of adornment is the only one, and its function lives with the other two ring verbs in `models/src/items/rings.rs`.


Potions, scrolls and wands
--------------------------

These three are consumables with a mechanic, so they take three edits. The pattern is identical for all three; only the names change.

1. **Append** a variant to the effect enum in `models/src/components.rs` -- `PotionEffect`, `ScrollEffect` or `WandEffect`.

2. **Add the row** in `POTIONS`, `SCROLLS` or `WANDS`.

       PotionDef { effect: PotionEffect::Levitation,
                   name: "potion of levitation", color: Color::Cyan },

       ScrollDef { effect: ScrollEffect::Genocide,
                   name: "scroll of genocide" },

       WandDef { effect: WandEffect::Digging, name: "wand of digging",
                 color: Color::DarkYellow, range: 8 },

   Potions draw as `!`, scrolls as `?` (always white), wands as `/`.
   A wand's `range` feeds the aiming reticle; a wand always spawns with
   `constants::wands::WAND_CHARGES`, full.

3. **Write the mechanic** as one arm of the matching function:

       apply_potion_effect(world, user, effect) -> bool   models/src/items/potions.rs
       apply_scroll_effect(world, user, effect)            models/src/items/scrolls.rs
       apply_wand_effect(world, user, target, effect)      models/src/items/wands.rs

   A potion's mechanic returns whether it visibly did anything.

> **Step 3 is the one the compiler makes you do.** All three of
> those functions are exhaustive matches over their effect enum, the
> same as `apply_trap_effect` is over `TrapEffect` -- no catch-all arm, so
> step 1's new variant will not build until step 3 gives it one. The
> error names the function and the missing variant, so there is no
> guessing which of the three you forgot.
>
> An explicit arm is still allowed to do nothing on purpose -- the scroll
> of blank paper, and the two potions that are only a taste, sit in
> do-nothing arms today, each variant named rather than caught by a
> wildcard (see
> `../explanation/data-driven-content.md`, "Where it does not reach").
> The guarantee is only that you *chose* nothing, not that you *forgot*
> to write something.

Runes take the same three edits, with `RuneEffect`, `RUNES` and `apply_rune_effect(world, user, effect)` in `models/src/items/runes.rs`:

    RuneDef { effect: RuneEffect::Ice, name: "rune of ice" },

A rune draws as `'`, always white. It is a scroll that goes inert when read instead of crumbling, and a staircase wakes it, so the mechanic is "what happens on each invocation" and never "what is left afterwards". `RuneEffect::Blank` is the one variant with no row: a wand of cancellation makes it. Only the player invokes a rune.

Wands only: if your wand should *not* open the aiming reticle -- it acts on the zapper or the room, like the wand of light -- add it to `WandEffect::needs_target` in `models/src/components.rs`.


> **Append enum variants; never insert or reorder them.** The save file
> encodes a variant as its position in the enum. Adding one at the end is
> safe for existing saves. Putting one in the middle silently turns every
> saved potion of healing into something else.

> **Potions, scrolls, wands and rings are always shown by their true
> name.** There is no cosmetic appearance and no per-effect identification
> to keep in step with the catalog -- add a row and it just works. Only
> equipment (weapons, armour, launchers) hides anything: its enchantment
> plus and cursed status, until `KnownQuality` says otherwise (see
> `../reference/components.md`).


Verify, whichever you added
---------------------------

    cargo build
    cargo run -p nihilurk -- -content | grep '<your name>'
    NIHILURK_SPAWN="<your name>" cargo run -p nihilurk
    cargo test --test content

The tests check that names are unique, that every row can be built by name, that what spawns keeps its name, and that every drop category can still produce something.

To use the item and see what it does without a terminal of your own, `play-through-a-pty.md` types the keys for you and prints the screen.


You do not have to register the item, update the loot table, teach the save file about it, or teach combat about it. Why none of that is needed is `../explanation/data-driven-content.md`.


Appendix: quick check
---------------------

1. Find your category in "At a glance".
2. Weapon, armour, coin, ammo, launcher, treat: add one row to its table in `models/src/catalog.rs`.
3. Ring: append a `RingEffect` variant at the end, then add the `RingDef::new` row.
4. Potion, scroll, wand, rune: append the effect variant at the end, add the row, write the arm in `apply_potion_effect`, `apply_scroll_effect`, `apply_wand_effect` or `apply_rune_effect`.
5. A wand with no reticle (it acts on the room or the zapper): add it to `WandEffect::needs_target`.
6. Never insert or reorder an enum variant; a save stores its position.
7. Run `cargo run -p nihilurk -- -content | grep '<name>'`.
8. Run `NIHILURK_SPAWN="<name>" cargo run -p nihilurk`, then `cargo test --test content`.
9. Fix the docs: the enum line in `../reference/components.md`, and for a wand its table row (`update-the-docs.md`).


See also
--------

  add-an-effect.md                 a property no component covers yet
  add-an-item-category.md          a whole new *kind* of item
  tune-rarity-and-depth.md         make it rare, or make it deep
  ../reference/content-tables.md   every field of every table
  ../explanation/data-driven-content.md   why rows carry components
