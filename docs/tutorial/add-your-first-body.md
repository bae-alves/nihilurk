Tutorial: add your first body
==============================

    Audience       Anyone who wants a new way to play the whole game,
                   not a new thing to meet in it. You have done
                   `add-your-first-monster.md`, or you are comfortable
                   enough to skip it.
    Prerequisites  A checkout, and a working `cargo`.
    Time           About twenty minutes.
    You will       Add a ninja: quick, quiet, poisoned steel, and a
                   fistful of daggers — built almost entirely out of
                   abilities and gear that already exist.

This is a lesson, not a recipe. When you want the short version, read `../how-to/add-a-body.md`.

```mermaid
%%{init: {'theme':'base','themeVariables':{
  'primaryColor':'#20242b','primaryTextColor':'#d7dae0',
  'primaryBorderColor':'#5c6370','lineColor':'#8a8f98',
  'fontFamily':'ui-monospace, SFMono-Regular, Menlo, monospace',
  'fontSize':'13px'}}}%%
flowchart LR
  S1["1 · find<br/>enum Body"]:::cold --> S2["2 · add<br/>the variant"]:::hero
  S2 --> S3["3 · borrow<br/>Quick + Stealthy"]:::magic
  S3 --> S4["4 · borrow<br/>Venomous"]:::magic
  S4 --> S5["5 · Sting in<br/>the spell bar"]:::magic
  S5 --> S6["6 · gear:<br/>equip_silently"]:::hero
  S6 --> S7["7 · -b ninja<br/>meet it"]:::peril
  S7 --> S8["8 · a staircase,<br/>not a reload"]:::cold
  classDef hero fill:#3a3418,stroke:#d7ba4a,color:#e8dfa8
  classDef peril fill:#3a1f1f,stroke:#c05050,color:#f0c8c8
  classDef magic fill:#2f2038,stroke:#a86fc0,color:#e6cdf0
  classDef cold  fill:#17323a,stroke:#4aa3c0,color:#bfe4f0
```


What you are about to learn
----------------------------

There is no `Class` trait and no `impl Ninja`. `body.rs` is one enum,
`Body`, with a hand-written function per variant that dresses the player —
`wear_lurk` is the one that already exists. A new body is a sibling of
that function, not a new shape.

The point of this lesson is that a body barely has to invent anything. Every
piece of the ninja — its speed, its quiet, its poison, even the fact that it
starts holding a weapon — is something the game already knows how to do for
somebody else. Writing the ninja is mostly *pointing* at those things.

By the end you will have a body that:

  * moves at the lurk's tempo and walks as quietly as the lurk does,
  * bites like a rattlesnake,
  * carries the spell Sting, the way the lurk carries Bide,
  * and spawns already wearing armour and a ring, and wielding a dagger —
    which is also your look at `equipment::equip_silently`, the function
    that hands somebody gear without a word being logged about it.


Step 1: find the enum
-----------------------

Everything a body *is* lives in one file:

    models/src/body.rs

Open it and find the enum:

    pub enum Body {
        #[default]
        Nihil,
        Lurk,
        Monster(&'static MonsterDef),
    }

Three answers today, and the module's own doc comment at the top of the file
explains why it's one enum and not a flag per body: a player is exactly one
of these, and the argument parser has nothing to cross-check. Read that
comment before you touch anything below it.


Step 2: add the variant
-------------------------

    pub enum Body {
        #[default]
        Nihil,
        Lurk,
        Ninja,
        Monster(&'static MonsterDef),
    }

`Body::name()` needs an arm too — it's what `-b` matches against, and what
an error about the body prints:

    pub fn name(self) -> &'static str {
        match self {
            Body::Nihil => "nihil",
            Body::Lurk => "lurk",
            Body::Ninja => "ninja",
            Body::Monster(def) => def.name,
        }
    }

And `wear` needs to route to a function you haven't written yet:

    pub fn wear(world: &mut World, player: Entity, body: Body) {
        match body {
            Body::Nihil => {}
            Body::Lurk => wear_lurk(world, player),
            Body::Ninja => wear_ninja(world, player),
            Body::Monster(def) => crate::monsters::wear_monster(world, player, def),
        }
    }

Build:

    cargo build

It won't compile — there is no `wear_ninja` yet. That's the whole rest of
the lesson.


Step 3: borrow the lurk's tempo and its quiet
-----------------------------------------------

Scroll down to `wear_lurk`. It inserts a bundle of components and then
grants a slice of effects through `grant_all`:

    const LURK_GRANTS: &[Grant] = &[
        Grant::of::<Lurk>(),
        Grant::of::<Lunges>(),
        Grant::of::<BuildsMomentum>(),
        Grant::of::<Stealthy>(),
    ];

`Stealthy` is a plain marker component — the same one a ring of stealth
grants, the same one `rings::STEALTH_RANGE` reads back out regardless of who
is wearing it. Nothing about it belongs to the lurk; the lurk just happens
to be the first thing that asked for it unconditionally instead of from a
ring. A ninja can ask for the same marker:

    const NINJA_GRANTS: &[Grant] = &[Grant::of::<Stealthy>()];

And the tempo is the same story. `wear_lurk` inserts:

    Speed::new(SpeedKind::Quick),

`SpeedKind::Quick` already exists — it's the lurk's tempo, half again as
fast as `Normal`. Start `wear_ninja` the same way `wear_lurk` starts,
borrowing both:

    fn wear_ninja(world: &mut World, player: Entity) {
        world.entity_mut(player).insert((
            Renderable {
                glyph: '@',
                color: Color::DarkGrey,
            },
            Fighter {
                hp: 8,
                max_hp: 8,
                power: 2,
                max_power: 2,
                power_bonus: 0,
                armor: 2,
                armor_bonus: 0,
            },
            Magic {
                points: 2,
                max_points: 2,
            },
            Speed::new(SpeedKind::Quick),
        ));
        grant_all(world, player, NINJA_GRANTS);
    }

The numbers are placeholders for now — 8 HP, nihil's own bare 2/2 dice, 2
Magic. You'll pull them out to `constants.rs` in the step where you decide
you're keeping this.


Step 4: borrow the rattlesnake's poison
------------------------------------------

`Venomous` is not a body's effect at all — it's the rattlesnake's, over in
`monsters.rs`:

    MonsterDef::row("rattlesnake", 'R', Color::DarkGreen, Chase, 6, 6, 0, 8, 0, 5)
        .grants(&[Grant::of::<Venomous>()]),

Nothing stops the same marker riding on a player instead of a bestiary row —
`Grant::of::<Venomous>()` doesn't know or care what kind of entity is
carrying it, the same way `Stealthy` didn't. Add it to the ninja's own
list:

    const NINJA_GRANTS: &[Grant] = &[Grant::of::<Stealthy>(), Grant::of::<Venomous>()];

Rebuild:

    cargo build

Still short one thing: `wear_ninja` is called from nowhere until you also
handle the spell and the gear.


Step 5: put Sting in the spell bar
--------------------------------------

`wear_lurk` ends by handing the lurk one spell it has to pay for:

    if let Some(mut spellset) = world.get_mut::<Spellset>(player) {
        spellset.slots.push(SpellEffect::Bide);
    }

Sting already exists — a venomed dart at range, 1 Magic, in
`items/spells.rs`. Give the ninja the same push, at the end of
`wear_ninja`:

    if let Some(mut spellset) = world.get_mut::<Spellset>(player) {
        spellset.slots.push(SpellEffect::Sting);
    }

It costs Magic like anybody else's copy of it would — a ninja is not
special-cased in `spell_cost`, the same way a dragon's innate Fireball
*is* (that one's free; see `../reference/cli-and-env.md`, "`-am <species>`").
Sting is learned, so it's paid for.


Step 6: gear, and `equip_silently`
--------------------------------------

Everything so far was an effect with nothing to spawn. Gear is different —
a dagger, a suit of armour and a ring are entities, and `wear_ninja` has to
build them the same way the floor stocks anyone else's kit: spawn at the
origin, strip the `Position` so it's carried and not lying on a tile
(`levels.rs::starting_kit` does the identical thing for nihil's mace and
ring mail), put it in the `Backpack`, then equip it.

`equipment::equip_silently(world, wearer, item)` is the function for that
last part — it puts the item on without narrating it, which is exactly
what you want for something the player is *born* wearing rather than
something they just picked up and pulled on.

    let origin = Position { x: 0, y: 0 };
    let strip = |world: &mut World, item: Entity| {
        world.entity_mut(item).remove::<Position>();
    };

    let armor = crate::catalog::spawn_armor(world, "leather armor", origin);
    strip(world, armor);
    let ring = crate::catalog::spawn_ring(world, RingEffect::Protection, origin);
    strip(world, ring);
    let daggers: Vec<Entity> = (0..5)
        .map(|_| {
            let dagger = crate::catalog::spawn_weapon(world, "dagger", origin);
            strip(world, dagger);
            dagger
        })
        .collect();

    if let Some(mut pack) = world.get_mut::<Backpack>(player) {
        pack.items.push(armor);
        pack.items.push(ring);
        pack.items.extend(daggers.iter().copied());
    }
    equip_silently(world, player, armor);
    equip_silently(world, player, ring);
    equip_silently(world, player, daggers[0]);

Five daggers: one wielded, four spare in the pack — a dagger is also
`.missile(4)`, built to be thrown, so the spares are a ninja's ammunition as
much as a backup blade. `equip_silently` refuses quietly if a slot is
already full or the item has nowhere to go (`Equipped` names its own slot),
so the order above — armour, then ring, then the first dagger — is safe to
follow literally.

Build:

    cargo build

It compiles. `-b` doesn't know the word "ninja" yet — that's the last wire.

Add a line in `engine/src/main.rs`, next to the two that are already there:

    match (flag, name.as_str()) {
        ("-b", "nihil") => body = Some((models::Body::Nihil, "-b")),
        ("-b", "lurk") => body = Some((models::Body::Lurk, "-b")),
        ("-b", "ninja") => body = Some((models::Body::Ninja, "-b")),
        ("-b", _) => unknown_body = Some((name.clone(), "-b")),
        ...
    }


Step 7: meet it
-----------------

    cargo build
    cargo run -p engine -- -b ninja

You wake up quick, quiet, venomous, holding a dagger, wearing leather and a
ring of protection, with four spare daggers and Sting in your spell bar.
Open the pack (`i`) and look — the armour and ring show as worn, the same
as nihil's ever do.

Fight something and watch the log: "Venom courses through the <name>
— its strength ebbs away," the same sentence a rattlesnake's bite would
read against you, just turned around to name what you bit instead.

That line didn't always exist. Writing this lesson turned up the gap:
`abilities::venomous_bite` used to print its sentence only when the
*victim* of the bite was the player — the one direction a real rattlesnake
ever attacks in — so a bite the player landed on a monster drained it in
total silence. Borrowing a monster's ability borrows its code exactly, and
that code had only ever been asked to speak in one direction. Fixed now
(`models/src/abilities.rs`, alongside the identical gap in the vampire's
`vampiric_drain`), but it's worth carrying the lesson forward: reusing an
effect is trusting whatever it was actually tested against, not what its
doc comment claims it does.


Step 8: the part a body cannot skip
---------------------------------------

It is tempting to guess this bug lives behind a save and reload, the way a
lurk's identity does. It doesn't — try it first, so you believe the actual
answer:

    cargo run -p engine -- -b ninja -ns
    # play a turn or two, quit, then:
    cargo run -p engine -- -b ninja -ns

You come back quick, quiet and venomous, exactly as you left. `Speed.kind`,
`Stealthy` and `Venomous` are all ordinary saved state — a plain field and
two rows in the `EFFECTS` ledger — and none of them need the body to be
*identified* to round-trip correctly. A reload is not where this breaks.

**A staircase is.** Find one and take it (`>`):

You come back Normal-paced. Not hasted, not slowed — the HUD's tempo badge
is simply gone, because `Normal` prints nothing. Nothing else about you
changed: still quiet, still venomous, still holding the same daggers.

Here's why, and it's narrower than it first looks. `crate::conditions::clear_player_conditions`
runs on *every* staircase (and on a wand of cancellation), and it always
sets `Speed.kind` back to `crate::body::innate_tempo(world, player)` — "back
to the body's own tempo... it has no business lifting what the player is
made of," in its own comment. `innate_tempo` answers that question by
checking for a `Lurk` marker, then a `MonsterBody` marker, and defaulting to
`Normal` if it finds neither. The ninja has neither. So the very first
staircase — not a reload, not anything special — asks "what is this
creature's own tempo?", gets the only honest answer available ("nothing
marks it as anything"), and quietly demotes a body that was supposed to be
permanently quick.

`Stealthy` and `Venomous` don't have this problem only because nothing else
in the game ever *takes them away and asks what to give back* — a staircase
lifts a ring of stealth's temporary loan the same way it lifts a potion's,
but nothing analogous exists for these two, so their absence of an identity
marker never gets tested. Tempo is the one property in the whole body
system that is actively *restored* rather than merely *held*, which is
exactly why it's the one that exposes a body with no identity of its own.

The fix is the one line the lurk's `Lurk` component exists for: give the
ninja a marker of its own that nothing else in the game would ever attach
to a player by accident (`Stealthy` and `Venomous` both fail that test —
a ring of stealth and a rattlesnake's bite can each hand a plain nihil the
same marker), and teach `innate_tempo` to read it the way it already reads
`Lurk` and `MonsterBody`. Picking that marker, and confirming it survives a
save the way `Lurk` does, is left as the next step — exactly the shape
`../how-to/add-a-body.md` describes it in.


Step 9: keep it or drop it
-----------------------------

If you like the ninja, decide on its save marker and finish the job. If
this was a dry run:

    git checkout models/src/body.rs engine/src/main.rs


What you actually learned
----------------------------

  * A body is a hand-written function, not a table row — `../how-to/add-a-monster.md`'s
    "one line in one table" does not apply here. What *does* carry over is
    reuse: almost nothing a body needs has to be invented.

  * An effect marker doesn't belong to whoever you first saw it on.
    `Stealthy` is the lurk's in the same sense `Venomous` is the
    rattlesnake's — which is to say, not really. Both are just components
    a `Grant` can hand to any entity at all.

  * Gear a creature is *born* with is built exactly like gear the floor
    hands out: spawn it, strip its `Position`, and either put it in a
    `Backpack` or (for something worn from the first turn) also run it
    through `equip_silently`.

  * A body needs an identity marker for reasons sharper than "does it
    survive a save" — ordinary saved state survives a save regardless.
    It's the code that actively *restores* something (`innate_tempo`,
    read on every staircase) that needs to ask "what is this creature,
    really" — and a body that never answers that question gets the
    default answer, silently, the first time anything asks. Borrowing an
    effect for its *behaviour* and borrowing one for its *identity* are
    two different questions, and the ninja above answered only the first.


Where to go next
-------------------

  ../how-to/add-a-body.md            the short version, as a recipe — including the save marker this tutorial left undone
  add-your-first-monster.md          the same kind of lesson, for a bestiary row
  ../how-to/add-an-effect.md         where `Stealthy` and `Venomous` actually come from
  ../reference/cli-and-env.md        `-b` and `-am`, from the player's side
  ../reference/components.md         `Body`, `MonsterBody`, `StartingBody`
