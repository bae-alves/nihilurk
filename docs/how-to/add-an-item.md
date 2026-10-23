How to add an item
==================

    Audience       Content author.
    Prerequisites  You know where `models/src/items/catalog.rs` is. If not,
                   do `../tutorial/add-your-first-item.md` first.
    Result         A new item that drops on floors and survives a save.
                   Equipment (weapons, armour, launchers, rings) hides
                   its enchantment plus and curse until worn or
                   identified; every other kind is always shown by its
                   true name.

One category per `DROPS` row. Most are a single row. Potions, scrolls, wands and runes are a row plus the handler it names, and a deck's cards are a table of their own. Find your category below and follow that recipe only.


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
| Ring      | `RINGS`     | yes        | --                               |
| Potion    | `POTIONS`   | no         | a handler in `items/potions.rs`, named by the row |
| Scroll    | `SCROLLS`   | no         | a handler in `items/scrolls.rs`, named by the row |
| Wand      | `WANDS`     | no         | a zap handler in `items/wands.rs`, named by the row |
| Deck      | `DECKS`     | yes        | a new card: a `CARDS` row and its `play` handler in `items/decks.rs` |
| Rune      | `RUNES`     | no         | a handler in `items/runes.rs`, named by the row |

Every table is in `models/src/items/catalog.rs`. Every mechanic is in `models/src/items/`, one file per kind: `potions.rs`, `scrolls.rs`, `wands.rs`, `runes.rs`, `rings.rs`, `decks.rs`, `pickups.rs`, `throwing.rs` (`models/src/items/mod.rs` itself is `item_system`, which plans a use and hands it to the row's handler).


Row-only categories
-------------------

### Weapon

    WeaponDef::new("mace", Color::DarkGrey, 6),

`new(name, colour, power_die)`. The die is what the weapon is worth in a swing: damage rolls `1d[power_die]`. Draws as `)`, worn in the hand slot.

Chain `.missile(die)` if it is built to be thrown -- it then rolls that die on impact, is spent on what it hits, and can never be caught out of the air. Chain `.piercing()` if a throw should run the whole line instead of stopping at the first body.

    WeaponDef::new("javelin", Color::DarkYellow, 5).missile(7).piercing(),

Chain `.returning()` if the throw should strike the first creature and fly home: into the pack, and back into the hand if the weapon was wielded. Leave off `.missile()` -- a projectile is spent on what it hits.

    WeaponDef::new("boomerang", Color::DarkYellow, 5).returning(),

Chain `.hits(n)` after `.returning()` and one throw strikes `n` creatures in all, each for full damage: the first in its way, then the nearest other enemy -- one the player can see, or for a monster thrower, any in throwing range with a clear line. The moon blade's row reads `.hits(3)`: hit, hit, hit, home.

    WeaponDef::new("moon blade", Color::Cyan, 3).returning().hits(3),

Any weapon can be thrown. `.missile()` is the difference between a purpose-built missile and a hurled lump of metal.

Chain `.reach(n)` for a weapon aimed with its own reticle (`v`) instead of a walk into the target's tile, `n` tiles out -- a bardiche, a whip. Add `.reach_piercing()` if the strike should run the whole line rather than stopping at the first body, the melee twin of `.piercing()`.

Chain `.grants(&[Grant::of::<SomeMarker>()])` for a trick the weapon lends its wielder while it's in hand -- exactly a ring's own `.grants()`, and read by the same places a ring's effects are: an `ABILITIES` row keyed on `Moment::OnHit` for something that fires when a blow lands, or a direct `world.get::<SomeMarker>(attacker)` probe for something checked elsewhere (the battle axe's `Cleaves`, checked once from `models::melee_attack`). This is also how a weapon-only marker stays invisible to `combat/` and `engine/` alike -- see `crate::effects`'s module doc.

Chain `.on_wear(OnWear(some_fn))` for a one-shot fired the instant it's wielded -- the staff's "You're a wizard now!" -- the same mechanism a ring of adornment's flourish uses. `.on_doff(OnDoff(some_fn))` is its mirror, fired when the weapon is deliberately put away again ("You're no longer that magical."), and it is worth adding only when what the weapon lends is invisible on the HUD: the player has no other way to notice it leaving. It fires from `equipment::toggle_equipped` alone, never from `force_unequip`, so dropping it, being disarmed of it and dying with it say nothing.

    WeaponDef::new("bardiche", Color::Grey, 7).reach(2).reach_piercing(),
    WeaponDef::new("garrote", Color::DarkGrey, 0).grants(&[Grant::of::<VorpalOnCondition>()]),

### Armour

    ArmorDef { name: "brigandine", color: Color::Grey, armor_die: 6, grants: &[] },

`armor_die` is what wearing it adds to the defence roll: `1d[armor_die]`. Draws as `]`, worn on the body. `cargo run -p nihilurk -- -content` lists the suits already there; their rows give the range a new one should sit in.

`grants` lends marker effects while it is worn, the same as a weapon's `.grants()`. Spikemail's `&[Grant::of::<Spiked>()]` is read by an `ABILITIES` row at `Moment::OnStruck`, the moment a blow lands on the bearer.

### Coin

    &CoinDef { name: "electrum coin", color: Color::DarkYellow,
               apply: pickups::ledger, helps: |_, _| true,
               amount: 2500, treasure: true, weight: 10 },

Coins are the pickup category: never carried, spent where they lie. A row is `name`, `color`, an `apply` that works the coin on the taker, a `helps` that says whether taking it would do anything, one `amount` that `apply` reads (points, hit points, afflictions lifted), and `treasure` when `amount` also goes into the score. Adding a *kind* of coin means writing the two handlers in `items/pickups.rs`; adding another coin of an existing kind is one row. Draws as `$`.

### Ammunition

    // Bolts: what a crossbow looses.
    &AmmoDef { name: "bolt", color: Color::Grey, die: IMPROVISED_THROW_DIE, launched_die: 5, launched_by: Grant::of::<FireQuarrel>(), on_blood: |_, _| {} },

The row goes in `AMMO`, like every catalog row: inline, with no name of its own in Rust.

`die` is what one rolls hurled by hand (ammunition is no better thrown than a mace is: `IMPROVISED_THROW_DIE`); a wielder carrying the `launched_by` effect rolls `launched_die` instead. `on_blood` runs on a victim each hit draws blood from: the blowdart names `throwing::sap`, which saps the victim's power. Write a fn in `items/throwing.rs` for a new trick. Stacks up to `constants::items::STACK_LIMIT` per pack slot, and arrives from the dungeon floor in bundles of `constants::loot::AMMO_BUNDLE_MIN..=AMMO_BUNDLE_MAX`.

If your ammunition answers to a launcher that does not exist yet, you need a new effect for the pair to meet at -- see `add-an-effect.md`.

### Launcher

    LauncherDef { name: "sling", color: Color::DarkYellow,
                  grants: &[Grant::of::<FireStone>()], melee_cap: 1 },

A launcher has no attack die and no armour die. All it does is put an effect on whoever holds it; ammunition that names the same effect is loosed rather than lobbed. Draws as `}`.

`melee_cap` is the most it is worth swung at something, whatever the dice or the enchantment say; every launcher in `LAUNCHERS` sets the same small cap. It is the price of the hand: a launcher fills the slot a sword would have, and swapping back (`x`) costs a turn.

Neither half knows the other exists. That is why a sling is one row here and one row in `AMMO`.

### Treat

    // A bone: for a creature without hands.
    &TreatDef { name: "bone", color: Color::White, takes: crate::companion::has_no_hands },

The row goes in `TREATS`. A treat is thrown at a monster as an offer of loyalty and is never used. `takes` says which monsters it is for: `companion::has_hands` (those with `ItemUser`), `companion::has_no_hands`, or a fn of your own in `companion.rs`. Stacks like ammo. Draws as `%`. What accepting one means is `models/src/creatures/companion.rs`.


Rings
-----

A ring is a modifier item, exactly like a sword. Most of them have no behaviour code anywhere -- they stack numbers through the same components combat already folds, and lend marker effects through `Grants`.

Add the row in `RINGS`:

    &RingDef::new("ring of fire resistance")
        .grants(&[Grant::of::<FireImmune>()]),

The `name` is the ring's identity: the save stores it, and a load that names no row is refused.

Available chains:

    .power_bonus(n)     flat modifier on the wearer's damage roll
    .armor_bonus(n)     flat modifier on the wearer's armour roll
    .throw_bonus(n)     flat modifier on everything they throw
    .max_hp_bonus(n)    max hp lent while worn (the ring of health)
    .grants(&[...])     marker effects lent while worn
    .on_wear(...)       a one-shot fired the instant it goes on

A bonus of zero attaches nothing, so an inert ring costs nothing at run time. Giving a ring a body is adding a chain to its existing row.

Reach for `.grants(...)` before anything else: if the property already exists as a marker some system asks about, you are done. If it does not, add the marker (`../how-to/add-an-effect.md`) and the one system that reads it -- that is how stealth, regeneration, teleportitis and maintain armor were wired, and none of them put a line in `catalog.rs` beyond the row.

`.on_wear(...)` is the last resort, for a ring whose effect is an *event* rather than a property: it takes an `OnWear(fn(&mut World, wearer, item))` and fires once, after the ring is worn and identified. The ring of adornment is the only one, and its function lives with the other ring verbs in `models/src/items/rings.rs`.


Potions, scrolls, runes and wands
---------------------------------

These are consumables with a mechanic.

Potions take two edits and no enum. Write the handler, `fn(&mut World, Entity) -> bool`, in `models/src/items/potions.rs`, and name it in the row's required `apply` field:

    &PotionDef::new("potion of levitation", Color::Cyan, potions::levitation),

The handler returns whether the dose visibly did anything: a potion thrown at a monster names itself only then. A verb that already exists can stand in for a handler: the potion of blindness names `conditions::blind`. The row is the potion: a `Potion` component points at its row, and the save stores the row's name. `PotionDef::new` takes the handler, so a row without one does not compile. A potion draws as `!`. The potion of thirst quenching is in `POTIONS` with `.never_drops()`: weight 0, so no floor rolls it, and only a wand of cancellation makes it.

Scrolls take two edits and no enum. Write the handler, `fn(&mut World, Entity)`, in `models/src/items/scrolls.rs`, and name it in the row; `ScrollDef::new` takes it, so a row without one does not compile:

    &ScrollDef::new("scroll of genocide", scrolls::genocide),

The reader is the player, or a monster the scroll was thrown at. A scroll every run needs drops twice as often with `.common()`, as identify and remove curse do. A scroll draws as `?`, always white. The scroll of blank paper is in `SCROLLS` with `.never_drops()`: only a wand of cancellation makes it, and its handler does nothing on purpose. Give the new scroll a flourish (`../explanation/the-feel-layer.md`, "What a scroll looks like").

Runes take two edits and no enum. Write the handler, `fn(&mut World, Entity)`, in `models/src/items/runes.rs`, and name it in the row's required `apply` field:

    &RuneDef::new("rune of ice", runes::ice),

The row is the rune: a `Rune` component points at its row, and the save stores the row's name. `RuneDef::new` takes the handler, so a row without one does not compile.

A rune draws as `'`, always white. It is a scroll that goes inert when read instead of crumbling, and a staircase wakes it, so the mechanic is "what happens on each invocation" and never "what is left afterwards". The blank rune is in `RUNES` with `.never_drops()`: only a wand of cancellation makes it. Only the player invokes a rune.

Wands take two edits and no enum. Write the zap handler, `fn(&mut World, Zap)`, in `models/src/items/wands.rs`, and name it in the row; `WandDef::new` takes it, so a row without one does not compile. The builder adds what the reticle, a throw and a `DualZap` read off the row:

    &WandDef::new("wand of fire", Color::Red, 8, BlastPalette::Fire, wands::fire)
        .attack().element(Element::Fire).bursts().wasted_on(wands::fireproof),

`.attack()` makes it a damage wand (thrown, the grenade; doubled by `DualZap`), `.element(..)` names what resists it, `.bursts()` paints the zap's blast on the reticle, `.unaimed()` skips the reticle (the wand of light), `.wasted_on(..)` lets the reticle pass over a creature it would not change, and `.thrown(Thrown::..)` picks what it does thrown (`Thrown::blast(f)` or one of the `Thrown` consts); the default is the small blast that changes nobody. A wand draws as `/`, its `range` feeds the reticle, and it always spawns with `constants::wands::WAND_CHARGES`, full. The wand of nothing is in `WANDS` with `.never_drops()`.


> **Never rename a row.** The save file stores an item's row by its
> `name`, and a load that names no row is refused. Adding a row anywhere
> in its table is safe for existing saves; renaming one breaks every save
> that holds it.

> **Potions, scrolls, wands and runes are always shown by their true
> name.** There is no cosmetic appearance and no per-effect identification
> to keep in step with the catalog -- add a row and it just works. Only
> equipment (weapons, armour, launchers, rings) hides anything: its
> enchantment plus and cursed status, until `KnownQuality` says otherwise
> (see `../reference/components.md`).


Verify, whichever you added
---------------------------

    cargo build
    cargo run -p nihilurk -- -content | grep '<your name>'
    NIHILURK_SPAWN="<your name>" cargo run -p nihilurk
    cargo test --test it content::

The tests check that names are unique, that every row can be built by name, that what spawns keeps its name, and that every drop category can still produce something.

Then run the whole suite. `content::` only reads the tables, and a new row changes what a seeded floor holds:

    cargo test

To use the item and see what it does without a terminal of your own, `play-through-a-pty.md` types the keys for you and prints the screen.


You do not have to register the item, update the loot table, teach the save file about it, or teach combat about it. Why none of that is needed is `../explanation/data-driven-content.md`.


Appendix: quick check
---------------------

1. Find your category in "At a glance".
2. Weapon, armour, ring, coin, ammo, launcher, treat: add one row to its table in `models/src/items/catalog.rs`.
3. A ring's row is `&RingDef::new(name)` and its chains.
4. Potion, scroll: write the handler in `items/potions.rs` or `items/scrolls.rs` and add the row naming it. Wand: write the zap handler in `items/wands.rs` and add the row naming it. Rune: write the handler in `items/runes.rs` and add the row naming it.
5. A wand with no reticle (it acts on the room or the zapper): `.unaimed()` on its row.
6. Never rename a row; a save stores its name.
7. Run `cargo run -p nihilurk -- -content | grep '<name>'`.
8. Run `NIHILURK_SPAWN="<name>" cargo run -p nihilurk`, then `cargo test --test it content::`, then `cargo test`.


See also
--------

  add-an-effect.md                 a property no component covers yet
  add-an-item-category.md          a whole new *kind* of item
  tune-rarity-and-depth.md         make it rare, or make it deep
  ../explanation/data-driven-content.md   why rows carry components
