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

2. **Add a row to `EFFECTS`**, pairing a stable id with the component:

       effects! {
           ...
           "fire_quarrel"   => FireQuarrel,
           "poison_immune"  => PoisonImmune,      // <- your row
       }

   The id is what a save file stores. Write it out; never derive it from
   the type name, and never rename it once saves exist -- the same rule a
   bestiary row lives by. Rows may be reordered and retired freely,
   because nothing about a row's *position* means anything.

   An effect that ends on its own gets the line the player reads when it
   does, in brackets after the type:

       "asleep" => Asleep ["You shake off the drowsiness and come to."],

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


> **Never rename an id.** The id, not the row's position, is what a save
> file stores, so renaming one is renaming the thing on disk: every
> existing save comes back without that effect. Reordering rows and
> retiring them are both safe, and there is no ceiling on how many there
> can be -- the `u64` bitset that capped the list at 64 is gone.

> **An id a save names and the build has no row for is dropped**, counted,
> and reported to the player in one line at the end of the load. Losing one
> property beats losing the run. This is what saving by name buys: with
> positions, a retired row shifted every later effect in every save and
> nothing could have told, because every index was still a valid index.

> **An effect missing from `EFFECTS` half-works.** It will attach, and it
> will do its job for the rest of the session. It will not be saved, will
> not come back on load, and cannot be cancelled. This is the quiet
> failure to look for when an effect works until you reload.
> `models/tests/abilities.rs` catches it for anything armed by an ability
> row; nothing catches it for an effect only a mechanic reads.


Making an effect act on its own
-------------------------------

Most effects are answers to a question something else asks. Some act by themselves, and `ABILITIES` in `models/src/abilities.rs` is all of those: one list, one row per ability, naming the `Moment` it fires at.

    models/src/abilities.rs     ->  ABILITIES

    Ability {
        effect: Grant::of::<AggravatesMonsters>(),
        when: Moment::EachTurn(0.10),
        player_only: false,
        action: |w, e, _| crate::items::aggravate_all_monsters(w, e),
        flavour: Some("You yip! The whole floor turns your way."),
    }

`when` is the moment. The two commonest:

* **`Moment::EachTurn(chance)`** -- while you carry it, something keeps happening: at this probability, on every turn the bearer acts. `action` runs on the bearer and reports whether it actually did anything; a ring of regeneration wins its coin flip every other turn, and most of those turns there is nothing wrong with the player to mend, so it returns `false` and stays silent.
* **`Moment::OnHit { glancing, lethal }`** -- the attacker's magic doing something to what it just hit. `effect` is the marker on the **attacker**, `action` runs as `(attacker, target)`, and the two flags are the only gating there is: does a glancing scrape count (acid says yes, a charm that needs skin says no), and does the killing blow count (there is no point charming a corpse).

Three rarer moments round out the enum: `OnDamaged` (the bearer was hurt and lived), `OnTargeted` (the player turned their attention on the bearer, whether or not the blow that follows lands -- a gorgon's gaze), and `InsteadOfAttacking(chance)` (the bearer would rather do this than swing -- a dragon's fireball). Adding a moment nobody has yet is a `Moment` variant and one arm in whatever drives it; adding an ability at a moment that already exists is one row here.

`flavour` is logged only when the bearer is the player *and* `action` returned `true`, so write it in the second person. `player_only` marks a row as the player's own trick -- a monster that steals or catches the weapon behind it still fights the plain way.

The row names the effect with the same `Grant` handle everything else uses, so the ability never learns what granted it. A ring grants it today; a cursed blade could grant it tomorrow and the behaviour would follow, untouched.

`ability_system` drives `EachTurn` and `InsteadOfAttacking` at the **tail** of the turn schedule (after `ai`, before `visibility_system`), which is deliberate: a passive that *moves* its bearer -- teleportitis -- lands the jump at the top of the bearer's next turn, so the player sees where they ended up and acts from there before anything else moves. `combat::resolve_attack` fires every `OnHit` row for a hit that dealt damage and has no idea what is in it -- which is why the aquator's corrosion and a scroll of monster confusion's charm are rows here rather than special cases in that function.


Making an effect fire once, when gear goes on
---------------------------------------------

A `Grant` is a property held while the gear is worn. An *event* at the moment of wearing is a different component, and it is not in `EFFECTS`:

    #[derive(Component, Clone, Copy)]
    pub struct OnWear(pub fn(&mut World, Entity, Entity));
    pub struct OnDoff(pub fn(&mut World, Entity, Entity));

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

Modifiers are not in `EFFECTS`; they are saved as the values they are.


Verify
------

    cargo build
    cargo test                   # save round-trips: tests/saveload.rs
    NIHILURK_SPAWN="<a thing that grants it>" cargo run -p engine

If your effect should survive a reload, the test worth copying is in `models/tests/wands.rs`: the one named `cancellation_strips_the_magic_but_leaves_the_creature` exercises the grant / probe / revoke path end to end.


See also
--------

  add-a-monster.md                 .grants(...) on a bestiary row
  add-an-item.md                   .grants(...) on a ring or launcher
  ../reference/content-tables.md   the effect registry as a table
  ../explanation/the-feel-layer.md  giving it a look, and the rules for one
  work-with-the-ecs.md             reaching the entity you want to change
  ../explanation/data-driven-content.md   why effects are components
  ../explanation/ecs-in-nihilurk.md    what a component is allowed to be
