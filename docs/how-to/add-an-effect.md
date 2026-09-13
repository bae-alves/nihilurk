How to add an effect
====================

    Audience       Content author reaching past the tables, or an
                   engine developer.
    Prerequisites  You have added a row (`add-a-monster.md` or
                   `add-an-item.md`) and want it to do something no
                   existing component covers.
    Result         A property that a monster can be born with, a ring
                   can lend, a wand of cancellation can strip, and the
                   save file remembers.

An effect is a component on the creature, and both the dragon born fire-immune and the player wearing a ring of fire resistance carry the same one. (Why that is the whole design: `../explanation/data-driven-content.md`.)

Everything here lives in:

    models/src/effects.rs


Which kind do you need?
-----------------------

    Marker      A yes/no property. "Fire does nothing to it."
                "It sees invisible things." Read as a query filter.

    Modifier    A number that stacks across everything equipped.
                "+2 armour." "+2 on throws."

    Cap         A ceiling the dice cannot beat. "At most 1 damage in
                melee." Folded with `min`, not `+`.

Markers are the common case. Take that branch unless your property is literally an integer that adds up, or a limit that overrides one.

There is one cap today (`MeleeCap`, carried by bows and crossbows) and it is read by one line of `resolve_attack`. It is written out by hand below the `modifiers!` rows rather than being one of them, because ceilings fold with `min` and the generated rows all fold with `+`. A second cap would be the moment to generalise that the way `equipped_total` is generalised over `Modifier`; see `melee_cap` in `models/src/effects.rs`.


Adding a marker effect
----------------------

1. **Declare the component** in `models/src/effects.rs`, next to its relatives. It must be a unit struct deriving exactly this set:

       /// Poison does nothing to this creature.
       #[derive(Component, Default, Clone, Copy)]
       pub struct PoisonImmune;

   `Default` is not optional -- it is how a `Grant` attaches the thing
   without knowing its type.

2. **Append it to `EFFECTS`**, at the end:

       pub const EFFECTS: &[Grant] = &[
           ...
           Grant::of::<FireQuarrel>(),
           Grant::of::<PoisonImmune>(),      // <- appended
       ];

3. **Hand it out** from a row. Any of these, or all of them:

       // a species born with it
       MonsterDef::row("basilisk", ...).grants(&[Grant::of::<PoisonImmune>()]),

       // a ring that lends it while worn
       RingDef::new(RingEffect::PoisonResistance, "ring of poison resistance")
           .grants(&[Grant::of::<PoisonImmune>()]),

       // a launcher that lends it while held
       LauncherDef { name: "sling", color: ...,
                     grants: &[Grant::of::<FireStone>()] },

4. **Ask about it** where it matters. It is an ordinary component, so:

       // in a query
       Query<&Viewshed, With<SeesInvisible>>

       // ad hoc
       if world.get::<PoisonImmune>(target).is_some() { return; }

       // through the handle, when you only have a Grant
       grant.probe(world, entity)

That is the whole of it. Equipping and unequipping, saving and loading, and the wand of cancellation all work immediately, because all three walk `EFFECTS` rather than a list of special cases.


> **`EFFECTS` is append-only, and it holds 32.** An effect's index in that
> array is the bit it occupies in the save file. Reordering it rewrites
> the meaning of every existing save. And an `EffectSet` is a `u32`, so
> the 33rd marker effect will need a wider type -- there are 16 today.

> **An effect missing from `EFFECTS` half-works.** It will attach, and it
> will do its job for the rest of the session. It will not be saved, will
> not come back on load, and cannot be cancelled. This is the quiet
> failure to look for when an effect works until you reload.


Making an effect act on its own
-------------------------------

Most effects are answers to a question something else asks. Some act by themselves, and there are two moments they can pick. Both are tables in `models/src/abilities.rs`, and both answer the same two questions: *when* does this fire, and *what* does it do.

**Every turn** -- while you carry it, something keeps happening:

    models/src/abilities.rs     ->  PASSIVE_ABILITIES

    PassiveAbility {
        effect: Grant::of::<AggravatesMonsters>(),
        chance: 0.10,
        action: crate::items::aggravate_all_monsters,
        flavour: "You yip! The whole floor turns your way.",
    }

`chance` is the probability it fires on each turn its bearer acts; `action` is the mechanic, run on the bearer, returning whether it actually did anything; `flavour` is logged only when the bearer is the player *and* the action returned `true`, so write it in the second person. Return `false` for a no-op -- a ring of regeneration wins its coin flip every other turn, and most of those turns there is nothing wrong with the player to mend.

The row names the effect with the same `Grant` handle everything else uses, so the ability never learns what granted it. A ring grants it today; a cursed blade could grant it tomorrow and the behaviour would follow, untouched.

The system runs at the **tail** of the turn schedule (after `ai`, before `visibility_system`), which is deliberate: a passive that *moves* its bearer -- teleportitis -- lands the jump at the top of the bearer's next turn, so the player sees where they ended up and acts from there before anything else moves.

**On a blow that lands** -- the attacker's magic doing something to what it just hit:

    models/src/abilities.rs     ->  ON_HIT_ABILITIES

    OnHitAbility {
        effect: Grant::of::<RustsArmor>(),
        on_glancing: true,
        on_lethal: true,
        action: corrode,
    }

`effect` is the marker on the **attacker**; `action` is run as `(attacker, target)`. The two booleans are the only gating there is: does a glancing scrape count (acid says yes, a charm that needs skin says no), and does the killing blow count (there is no point charming a corpse). `combat::resolve_attack` fires the table for every hit that dealt damage and has no idea what is in it -- which is why the aquator's corrosion and a scroll of monster confusion's charm stopped being two special cases in that function.


Making an effect fire once, when gear goes on
---------------------------------------------

A `Grant` is a property held while the gear is worn. An *event* at the moment of wearing is a different component, and it is not in `EFFECTS`:

    #[derive(Component, Clone, Copy)]
    pub struct OnWear(pub fn(&mut World, Entity, Entity));

`equipment::toggle_equipped` fires it with `(wearer, item)` after the item is worn, named and known, and the function owns everything that follows -- including tagging the item `Consume` if wearing it is what spends it (`item_system` re-checks for that tag after the toggle and destroys the item instead of stowing it). The ring of adornment is the only thing in the game that uses it. A row attaches it the way it attaches grants, and `saveload` reads it back off the row.


Adding a numeric modifier
-------------------------

Rarer, and a bigger change, because a number has to be added *somewhere* specific -- there is no generic place a new number belongs.

1. Add a row to the `modifiers!` table in `effects.rs`, naming the component and the `Loadout` field it folds into:

       modifiers! {
           ...
           /// Flat modifier on how far the bearer can see.
           SightBonus => sight_bonus,
       }

   That row is the whole declaration. It gives you `pub struct
   SightBonus(pub i32)` implementing `Modifier`, a `Loadout.sight_bonus`
   field, and the line of `Loadout::absorb` that sums it -- so the
   single-pass fold cannot be left holding five of your six modifiers.
   The rows are the only place a modifier is named.

2. Attach it from rows the way `ArmorBonus` is attached -- see `RingDef::armor_bonus` and `insert_modifier`, which skips a zero so an inert row costs nothing.

3. **Read it where it applies**:

       equipped_total::<SightBonus>(world, entity)   // one number
       loadout(world, entity).sight_bonus            // already taking the fold

   Either sums the component across everything the entity has equipped
   plus anything it carries itself. Take the first when a caller wants one
   number and the second when it is already walking the gear for others
   -- combat and the HUD both want the whole `Loadout`, and a sixth
   `equipped_total` call there would be a sixth walk over the same handful
   of items.

   This is the step no framework can do for you: `PowerBonus` is read in
   combat, `ThrowBonus` in the throw code, and your new one has to be read
   by whoever owns the number it modifies. A generated field nobody reads
   is inert, which is a visible nothing rather than a wrong number.

Modifiers are not in `EFFECTS` and are not bits in a save; they are saved as the values they are.


Verify
------

    cargo build
    cargo test                   # save round-trips: tests/saveload.rs
    ROOG_SPAWN="<a thing that grants it>" cargo run -p engine

If your effect should survive a reload, the test worth copying is in `models/tests/wands.rs`: the one named `cancellation_strips_the_magic_but_leaves_the_creature` exercises the grant / probe / revoke path end to end.


See also
--------

  add-a-monster.md                 .grants(...) on a bestiary row
  add-an-item.md                   .grants(...) on a ring or launcher
  ../reference/content-tables.md   the effect registry as a table
  ../explanation/the-feel-layer.md  giving it a look, and the rules for one
  work-with-the-ecs.md             reaching the entity you want to change
  ../explanation/data-driven-content.md   why effects are components
  ../explanation/ecs-in-roog.md    what a component is allowed to be
