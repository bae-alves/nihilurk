Reference: content tables
=========================

    Audience       Anyone editing content. Look things up here; do not
                   read it end to end.
    Prerequisites  None.
    Status         Describes the tables as they are in the source. If
                   this page and the source disagree, the source is
                   right and this page is a bug.

Every table in the game, every field, every default.


Where everything is
-------------------

| Table               | File                       | Row type       |
|---------------------|----------------------------|----------------|
| `BESTIARY`          | `models/src/monsters.rs`   | `MonsterDef`   |
| `POTIONS`           | `models/src/catalog.rs`    | `PotionDef`    |
| `SCROLLS`           | `models/src/catalog.rs`    | `ScrollDef`    |
| `WANDS`             | `models/src/catalog.rs`    | `WandDef`      |
| `WEAPONS`           | `models/src/catalog.rs`    | `WeaponDef`    |
| `AMMO`              | `models/src/catalog.rs`    | `AmmoDef`      |
| `LAUNCHERS`         | `models/src/catalog.rs`    | `LauncherDef`  |
| `ARMORS`            | `models/src/catalog.rs`    | `ArmorDef`     |
| `RINGS`             | `models/src/catalog.rs`    | `RingDef`      |
| `COINS`             | `models/src/catalog.rs`    | `CoinDef`      |
| `TRAPS`             | `models/src/traps.rs`      | `TrapDef`      |
| `DROPS`             | `models/src/spawn.rs`      | `DropCategory` |
| `EFFECTS`           | `models/src/effects.rs`    | `Grant`        |
| `PASSIVE_ABILITIES` | `models/src/abilities.rs`  | `PassiveAbility` |
| `ON_HIT_ABILITIES`  | `models/src/abilities.rs`  | `OnHitAbility` |
| `FLAGS`             | `models/src/pride.rs`      | `PrideFlag`    |

For the live contents of any of them:

    cargo run -p engine -- -content


BESTIARY -- MonsterDef
----------------------

Constructor: `MonsterDef::row(name, glyph, color, movement, hp, power,
power_bonus, armor, armor_bonus, min_depth)`, then optional chains.

| Field         | Type               | Def.    | Meaning                    |
|---------------|--------------------|---------|----------------------------|
| `name`        | `&'static str`     | --      | Unique; its save identity. |
| `glyph`       | `char`             | --      | One character on the map.  |
| `color`       | `Color`            | --      | See the palette below.     |
| `movement`    | `MovementType`     | --      | Static/Chase/Flee/Confused |
| `hp`          | `i32`              | --      | Hit points; also `max_hp`. |
| `power`       | `i32`              | --      | Attack die: `1d[power]`.   |
| `power_bonus` | `i32`              | --      | Flat, once, on that roll.  |
| `armor`       | `i32`              | --      | Defence die: `1d[armor]`.  |
| `armor_bonus` | `i32`              | --      | Flat, once, on that roll.  |
| `min_depth`   | `u8`               | --      | Shallowest floor it spawns.|
| `weight`      | `u32`              | `10`    | Rarity vs eligible pool.   |
| `grants`      | `&'static [Grant]` | `&[]`   | Effects it is born with.   |
| `invisible`   | `bool`             | `false` | Born unseeable.            |

Chains: `.grants(&[...])`, `.invisible()`, `.weight(n)`.

Combat: `damage = (1d[power] + power_bonus) - (1d[armor] + armor_bonus)`,
both sides rolled independently. Nothing ever misses.

Spawning a row attaches: `Name`, `Mob`, `Fighter`, `Renderable`,
`Position`, `Faction::Monster`, `Blood`, `Grants`, `Speed(Normal)`, plus
each granted effect component, plus `Invisible` if the row asked.

`MovementType::Aggravated { tx, ty }` exists but is applied at run time by
the scroll of aggravate monsters. Never put it in a row.

A species' whole special behaviour is its `grants`. The aquator is the
one whose grant reaches for the *player's gear* rather than the player:
`RustsArmor` means every blow it lands calls `equipment::corrode_armor`
on what it hit, taking a point off the worn armour's `ArmorBonus` (never
its die — ruined plate is still plate, and the plus can go negative). A
scroll of enchant armour mends it; a ring of maintain armor
(`SustainsArmor`) stops it; a wand of cancellation takes the corrosion
out of the aquator.


Item tables
-----------

Every row implements `ItemDef`, which supplies:

    fn name(&self) -> &'static str            required
    fn spawn(&self, world, pos) -> Entity      required
    fn weight(&self) -> u32                    default 10
    fn min_depth(&self) -> u8                  default 1
    fn spawn_as_loot(&self, world, rng, pos)   default: calls spawn

No item row overrides `weight` or `min_depth` today -- within a category
roog picks evenly on purpose.

### POTIONS -- PotionDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `PotionEffect` | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |
| `color`  | `Color`        |                                          |

Draws `!`. Attaches `Item`, `Potion`, `Consume`.
Mechanic: `apply_potion_effect` in `models/src/items/potions.rs`. That
match has no catch-all, so a new `PotionEffect` variant will not
compile until it has an arm -- same rule as `TrapEffect` above. An arm
is allowed to do nothing on purpose, though only two do today
(`FruitJuice` and `Water`, which are a log line and a taste); the other
thirteen all bite.

The dials the arms read -- how much ceiling a dose of healing is worth,
how much power poison takes -- are `constants::potions`. Four arms reach
straight into `Fighter` (healing, extra healing, gain strength, poison,
restore strength), three hand off to `crate::conditions` (blindness,
confusion, paralysis -- and haste, via `hasten`), two tag things
`Detected`, one grants `SeesInvisible` `GrantedForFloor`, and
`RaiseLevel` calls `transition_level` upward (and, on Depth 1 with the
Element of Yoord in the pack, wins the run outright).

### SCROLLS -- ScrollDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `ScrollEffect` | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |

Draws `?`, always white -- a scroll has no colour field.
Attaches `Item`, `Scroll`, `Consume`.
Mechanic: `apply_scroll_effect` in `models/src/items/scrolls.rs`. That
match has no catch-all, so a new `ScrollEffect` variant will not
compile until it has an arm -- same rule as `TrapEffect` above.
Every row bites; `BlankPaper` is the only arm that does nothing, and it
says so.

Each one has a flourish of its own. The enchantments and monster
confusion throw `Particles::spark_burst` off the reader's tile — orange
for a plus biting into gear, magenta for a charm settling onto a pair of
hands (and again on the victim when the next blow spends it); food
detection uses the same burst in green. Scare monster, hold monster and
sleep flash `Particles::condition_mark` over each creature they caught
(`!`, `#`, `z`), staggered a beat apart so a roomful reads as a wave
crossing the room rather than every tile blinking at once — and the
lasting news is the renderer's status tint under the creature, which
holds for as long as the condition does.

The dials the arms read -- what one enchantment is worth, how long sleep
and hold last, how often sleep backfires -- are `constants::scrolls`.
Three arms go off across whatever the reader can see (scare monster,
hold monster, sleep, via `helpers::hostiles_in_view`), two reach into the
gear in a slot (the enchantments), one arms the reader's next blow
(monster confusion, spent by the `ON_HIT_ABILITIES` row below), two tag things
`Detected` -- food detection takes precisely what a potion of magic
detection rejects, so between them they find every item on the floor
exactly once, with the Element of Yoord deliberately turning up for both.

### WANDS -- WandDef

| Field    | Type           | Notes                                    |
|----------|----------------|------------------------------------------|
| `effect` | `WandEffect`   | Keys the mechanic; identity in saves.    |
| `name`   | `&'static str` |                                          |
| `color`  | `Color`        |                                          |
| `range`  | `i32`          | Feeds the aiming reticle. In use: 6, 8.  |

Draws `/`. Attaches `Item`, `Wand`, `Ranged`, `Battery`.
A floor drop rolls `2d6 + 1` charges (`roll_wand_charges`). Charge dice,
zap damage dice and both blast radii live in `models/src/constants.rs` →
`wands` (see `reference/constants.md`).
Whether zapping opens the reticle: `WandEffect::needs_target`, which is
true for everything except the wand of light.
Mechanic: `apply_wand_effect` in `models/src/items/wands.rs`. That
match has no catch-all, so a new `WandEffect` variant will not compile
until it has an arm -- same rule as `TrapEffect` above; every existing
variant already does something real. Throwing a wand is resolved in
`models/src/items/throwing.rs` (`resolve_wand_throw`), a narrower,
still-`_`-fallback dispatch over only the six utility effects a thrown
blast can carry (`apply_thrown_wand_effect`) -- it is not the
"unwired effect" checkpoint; `apply_wand_effect` is.

A zapped bolt (`fire_bolt` → `Particles::beam`) flickers white/its own
colour twice per cell rather than fading once, and lands with
`Particles::impact_sparks` — a brighter flash on the hit tile plus a small
ring of offset sparks around it, timed off `beam`'s returned flight time so
they pop right as the bolt arrives. Flashier and denser than the beam
alone, on every bolt-type wand (fire, cold, lightning, magic missile,
striking, drain life).

Teleport away/to (`teleport_entity_away`, `teleport_target_here`) leaves a
`Particles::poof` and a short `map::Smoke` puff (`TRANSMUTATION_SMOKE_TURNS`,
2 turns) where the creature stood — the same magenta signature the teleport
trap and the scroll of teleportation now leave. Polymorph (`polymorph_entity`) puffs
the same smoke in a small ring around the transformed creature's tile
(`leave_smoke_ring`), staggered so it reads as smoke rolling outward.

Every particle animation's frame pacing (a zap's beam, a blast's ripple, a
teleport's poof, the magic-mapping reveal wipe) scales with the `AnimRate`
resource, set once at startup from `-anim-rate` — see
`reference/cli-and-env.md`.

Neither a zap nor a throw can be aimed at the player's own tile: the
engine refuses it with "Great idea! But no." and no turn passes.

**Thrown**, a wand only bursts on *impact* — hitting a creature or a
wall, or flying its full `THROW_RANGE` leash. Lobbed into open floor
short of that, it just lands with its charges and its secret intact.

On impact (`resolve_wand_throw`) it spends every remaining charge at
once. The blast animation is coloured per wand (`blast_palette` →
`particles::BlastPalette`) and always opens on that palette's bright
first frame — a primary blast never reads as dark. On a light or
utility wand's throw (never an attack wand's — it returns before the
per-creature loop), every creature the blast actually caught also gets
a small, darker `Particles::secondary_burst` a beat after the primary
ripple passes its tile — purely cosmetic, confirming who the effect
landed on, no gameplay of its own.

Fire and cold blasts (zapped or thrown — both run through the one
`elemental_blast`) also billow a grey/white `Particles::smoke_burst`
across the blast cells a beat after the flames. Fire's smoke additionally
lingers on the map for `SMOKE_LINGER_TURNS` (4) real turns afterward,
DCSS-style — the `map::Smoke` resource, ticked once a turn by
`smoke_system`, ages every puff down and the renderer draws a grey `≈`
over any smoky tile currently in view (never on a tile an actor stands
on, like blood). Cold's puff is animation only; nothing persists.

Anyone `elemental_blast` kills outright is finished off right there
(`combat::finish_indirect_kill`) rather than left for `reaper_system`'s
next sweep, so their death burst knows the blast's centre and flings the
corpse radially outward through where they stood — an edge casualty gets
launched straight on away from the blast, not a random direction like an
ordinary indirect kill. A casualty standing exactly on the blast's own
centre has no "outward" to speak of, so it falls back to the usual random
fling.

- *Attack* wands (fire, cold, lightning, magic missile, striking, drain
  life — `is_attack_wand`) throw the wide grenade: `GRENADE_RADIUS`, `d4`
  per remaining charge, armour-ignoring. Fire and cold carry their
  element (immunities apply); the rest are non-elemental.
- *Wand of light* throws the same wide `d4`-a-charge grenade, but instead
  of an element it **dazzles** every creature caught (`dazzle`) — a
  monster flips to `MovementType::Confused`, the player gains the
  `Confused` condition. "dazzle" in the log.
- *Utility* wands (polymorph, haste, slow, teleport away/to,
  cancellation) throw a small `BLAST_RADIUS` blast that deals **no
  damage** — the effect is the whole payload, worked on every creature
  caught (`apply_thrown_wand_effect`), thrower included.
- Wand of nothing: bursts in magenta/cyan confetti particles, no blast.

Effects that can land on the **player** (via a thrown blast): polymorph
logs "You feel like a new person"; haste/slow set the player's `Speed`;
dazzle gives `Confused`; teleport away runs the scroll-of-teleportation
relocation; teleport-to with no other target logs the "straight to
yourself" joke; cancellation is `cancel_player` — zeroes every
`PowerBonus`/`ArmorBonus` on weapons and armour, turns unread scrolls to
`BlankPaper` and potions to `Water`, and lifts every `Curse` without
destroying the item.

**Player conditions** (`Confused`, `Blind`, `Paralyzed`, and `Speed`
haste/slow) are treacherous: they never wear off with time. Only two
things clear them, both via `conditions::clear_player_conditions`, which
logs "You are no longer {}." for each: **using a staircase**
(`transition_level`) and **a wand of cancellation** (`cancel_player`).
The same call hands back whatever a potion lent for the floor
(`GrantedForFloor` — today, a potion of see invisible's sight). The HUD
shows them as 4-letter mnemonics (`FAST` cyan / `SLOW` green / `CONF`
magenta / `BLND` dark grey / `PARL` dark magenta), suppressing the score
line for space when any is lit.

While `Confused`, half of every walk or swing goes off in a random
direction ("You stumble foolishly"; `maybe_stumble` in
`engine/src/update.rs`), and fast movement, auto-explore and auto-fight
all refuse with "You are too confused for that right now."

While `Blind`, sight is cut to the 3x3, everything in it is painted
white, and every monster is `Hidden` — so auto-walk and auto-fight have
nothing to look at either. The monsters are *not* blinded back: `ai`
works off the view the player would have with their eyes open.

While `Paralyzed`, tempo drops to `Slow` and a
`potions::PARALYSIS_LOST_TURN_CHANCE` share of the remaining turns is
forfeited outright, rolled once per turn in `process_input_and_update`
before a key is read.

### WEAPONS -- WeaponDef

Constructor: `WeaponDef::new(name, color, power_die)`, then chains.

| Field        | Type           | Default      | Notes                     |
|--------------|----------------|--------------|---------------------------|
| `name`       | `&'static str` | --           |                           |
| `color`      | `Color`        | --           |                           |
| `power_die`  | `i32`          | --           | Damage rolls `1d[n]`.     |
| `thrown_die` | `i32`          | `power_die`  | Die rolled on impact.     |
| `projectile` | `bool`         | `false`      | Built to be thrown.       |
| `piercing`   | `bool`         | `false`      | Throw runs the whole line.|

Chains: `.missile(die)` sets `thrown_die` and `projectile`;
`.piercing()` sets `piercing`.

Draws `)`. Attaches `Item`, `Equipped::loose(Slot::Hand)`, `PowerDie`,
`ThrownDamage`, and `Projectile` / `Piercing` when asked.
A floor drop is enchanted (see below).

`Projectile` means three things at once: the throw ignores the target's
armour die, the missile is spent on what it hits, and nothing can catch
it. A non-projectile throw is blunted by armour and can be caught and
used against you.

### AMMO -- AmmoDef

| Field         | Type           | Notes                                   |
|---------------|----------------|-----------------------------------------|
| `name`        | `&'static str` |                                         |
| `color`       | `Color`        |                                         |
| `die`         | `i32`          | Rolled when hurled by hand.             |
| `launched_by` | `Grant`        | The effect that doubles the die.        |

Draws `)`. Attaches `Item`, `ThrownDamage`, `Projectile`, `LaunchedBy`,
`Stack { count: 1 }`. No `PowerDie` and no `Equipped` -- there is nothing
to wield and nothing to wear.

Stacks to `STACK_LIMIT` (13) per pack slot. A floor drop arrives as a
bundle of 3-12.

### LAUNCHERS -- LauncherDef

| Field       | Type               | Notes                              |
|-------------|--------------------|------------------------------------|
| `name`      | `&'static str`     |                                    |
| `color`     | `Color`            |                                    |
| `grants`    | `&'static [Grant]` | Effects lent to whoever holds it.  |
| `melee_cap` | `i32`              | Most it is worth swung. Both are 1.|

Draws `}`. Attaches `Item`, `Equipped::loose(Slot::Hand)`, `Launcher`,
`ThrowBonus(0)`, `Grants`, `MeleeCap`. Contributes no attack or armour die
at all -- its enchantment therefore lands on `ThrowBonus`.

`melee_cap` becomes a `MeleeCap` component, which clamps the wielder's
melee damage however the dice fell. A launcher takes the hand a sword
would have had; this is what that hand costs.

### ARMORS -- ArmorDef

| Field       | Type           | Notes                                  |
|-------------|----------------|----------------------------------------|
| `name`      | `&'static str` |                                        |
| `color`     | `Color`        |                                        |
| `armor_die` | `i32`          | Defence roll adds `1d[n]`. In use 2-9. |

Draws `]`. Attaches `Item`, `Equipped::loose(Slot::Body)`, `ArmorDie`.

### RINGS -- RingDef

Constructor: `RingDef::new(effect, name)`, then chains.

| Field         | Type                | Default | Notes                     |
|---------------|---------------------|---------|---------------------------|
| `effect`      | `RingEffect`        | --      | Identity for ident/saves. |
| `name`        | `&'static str`      | --      |                           |
| `power_die`   | `i32`               | `0`     | Rarely used.              |
| `power_bonus` | `i32`               | `0`     | `.power_bonus(n)`         |
| `armor_die`   | `i32`               | `0`     | Rarely used.              |
| `armor_bonus` | `i32`               | `0`     | `.armor_bonus(n)`         |
| `throw_bonus` | `i32`               | `0`     | `.throw_bonus(n)`         |
| `grants`      | `&'static [Grant]`  | `&[]`   | `.grants(&[...])`         |
| `on_wear`     | `Option<OnWear>`    | `None`  | `.on_wear(...)` — a one-shot fired the moment it goes on |

Draws `=`, always yellow. Attaches `Item`, `Ring`,
`Equipped::loose(Slot::Finger)`, each non-zero modifier, `Grants` when
non-empty, and `OnWear` when the row has one. A zero modifier attaches
nothing. `grants` and `on_wear` are both re-read from the row on load,
never stored in the save.

All twelve rings are live, and eleven of them are pure table — a modifier
combat already folds, or a marker some system already asks about:

| Ring | Row content |
|---|---|
| protection | `.armor_bonus(2)` |
| strength | `.power_bonus(2)`, grants `SustainsStrength` |
| perception | grants `SeesInvisible` |
| aggravate monster | grants `AggravatesMonsters` |
| dexterity | `.throw_bonus(2)` |
| increase damage | `.power_bonus(2)` |
| regeneration | grants `Regenerates` |
| slow digestion | grants `Sluggish` |
| teleportation | grants `Teleportitis` |
| stealth | grants `Stealthy` |
| maintain armor | grants `SustainsArmor` |
| adornment | `.on_wear(items::rings::ADORNMENT)` |

The only ring behaviour code in the tree is `models/src/items/rings.rs`,
and it holds three verbs, not a `match` on `RingEffect`: the adornment
flourish (which the victory climb also calls), the regeneration tick and
the teleportitis jump. Nothing outside that file asks which ring it has.

`do_it_with_style` is the flourish: eight `Particles::firework` blasts,
one on each tile around the player, `90ms` apart so the chain runs round
them, each in a colour drawn at random from `particles::GLORY_COLORS` —
the seven bright terminal colours, white excluded, because white is what
every other burst in the game opens on. Plus a `ShakeKind::Heavy` kick,
four lines of fanfare, and `score::double`. Two things call it: wearing
the ring, and `map::win_with_style` (both ways out of the dungeon — the
last stair and a potion of raise level on Depth 1). Queued like any other
animation, so the engine plays it out before the victory starfield.

### COINS -- CoinDef

Coins are the whole **pickup** category: an item that is never carried,
works where it lies, and is gone. See `components.md`, "Components --
items on the floor and in the pack".

| Field    | Type            | Notes                                          |
|----------|-----------------|------------------------------------------------|
| `name`   | `&'static str`  |                                                |
| `color`  | `Color`         |                                                |
| `effect` | `PickupEffect`  | Keys the mechanic; identity in saves.          |
| `amount` | `i32`           | The one dial. What it means is `effect`'s business: score for a treasure coin, hit points for the red one, afflictions lifted for the rosé. `0` for a row that needs no number. |

Draws `$`. Attaches `Item`, `Pickup`, and — for a treasure coin only —
`Value`, which is the component the score reads (the relic carries the
same one).

| Coin | Effect | What it does |
|---|---|---|
| gold | `Coin` | `amount` into the score |
| silver | `Coin` | the same, less of it |
| red | `Health` | heals up to `amount`, never past `max_hp` |
| blue | `Power` | refills up to `amount` magic points |
| rosé | `Cleanse` | lifts up to `amount` afflictions, worst first |
| green | `Strength` | gives back up to `amount` drained `power` |
| platinum | `Platinum` | the `Plated` promise |
| forge | `Forge` | the `Forged` promise |

Mechanic: `apply` in `models/src/items/pickups.rs`, an exhaustive match
with no catch-all — a new `PickupEffect` does not build until it does
something.

**A coin that would do nothing is not taken.** `pickups::would_help`
gates every one of them: a red coin at full health, a rosé coin with
nothing wrong with you, a platinum coin when you already hold the
promise. The coin stays on the floor, silently, and auto-explore skips it
too (`autoexplore::known_item_tiles`) until the day it would help.

**A full pack is no obstacle**, because there is nothing to find room for.
This is the one thing on the floor a full pack can still answer.

### The relic

Not a table -- one function, `spawn_element_of_yoord`. Draws `"` in
magenta, worth 25000, carries `Amulet`. Its name is the constant
`ELEMENT_OF_YOORD`. Spawned in place of the down-stair on floor 13.


TRAPS -- TrapDef
----------------

| Field         | Type          | Notes                                          |
|---------------|---------------|------------------------------------------------|
| `effect`      | `TrapEffect`  | Keys the mechanic; identity in saves.          |
| `name`        | `&'static str`| What `TrapEffect::label()` returns.            |
| `glyph`       | `char`        | `'^'` for all six.                             |
| `color`       | `Color`       | One per row.                                   |
| `weight`      | `u32`         | Rarity. All six are `10`.                      |
| `min_depth`   | `u8`          | All six are `1`.                               |
| `snare_turns` | `u32`         | Turns a bear / gas trap holds you; `0` otherwise. |

A trap entity carries `Name`, `Renderable`, `Position`, `Trap`, `Hidden`.
It is never an `Item` and never a tile type.

Mechanic: `apply_trap_effect` in `models/src/traps.rs`. That match has no
catch-all, so a new `TrapEffect` variant will not compile until it has an
arm. A snaring trap reads its duration straight off the row, so it stays a
one-file change.

Reveal style is rolled per trap at spawn, equal odds, not per row:
`Sight` / `Adjacent` / `Triggered`.

A trap set off *from a distance* -- a missile that lands on it, a wand's
blast that covers it -- has nobody standing on it to bite, so it bursts
instead: `detonate_trap` deals `TRICK_SHOT_DAMAGE_DICE d
TRICK_SHOT_DAMAGE_SIDES` (armour-ignoring) over the `TRICK_SHOT_RADIUS`
around its tile and then runs the mechanic once per creature caught.
Dials: `constants::traps`.

### Trick shots -- what a landing missile can set off

`traps::detonate_at(world, pos)` is the whole of it, called by
`items::throwing::resolve_throw` for every throw, whatever was thrown. It
returns a `TrickShot` saying what went off, or `None`.

| On the tile | Reach | Damage | Follow-up |
|---|---|---|---|
| a `Trap` | `TRICK_SHOT_RADIUS` | the trick-shot dice | the trap's own effect, per survivor |
| a `Pickup` (a coin) | `PICKUP_TRICK_SHOT_RADIUS` (double) | the same dice | **the coin's effect, paid to the shooter** (`pickups::claim_from_afar`) — a red coin heals them, a gold one pays them, a platinum one makes them its promise. Worked *before* the burst, so the shooter's own blast cannot take the healing back off them. No `would_help` gate: stepping over a coin is leaving it for later, shooting one is a decision, and a decision is allowed to be a waste |
| the `Amulet` (the Element of Yoord) | `PICKUP_TRICK_SHOT_RADIUS`, then `TRICK_SHOT_RADIUS` twice | the dice, once per burst | see below. The relic is never destroyed, moved or spent |

A shot that sets *anything* off with something other than ammunition —
a dagger, a potion, somebody's spare ring — logs "Very clever." Firing an
arrow into a trap is what arrows are for; doing it with the rest of your
kit is a choice.

**The ULTIMATE TRICK SHOT** (`traps::ultimate_trick_shot`) is what the
relic does when a missile comes down on it: one wide burst where it lies,
then a second burst centred on *every* creature that one caught, then a
third on one of them (the first in reading order). Each can catch somebody
the last one missed. All three burn in `BlastPalette::Ultimate` — white
through magenta to dark magenta, the only blast no wand can produce — and
the primary leaves a `Smoke` puff over every tile it covered. Only the
first burst shouts `BAM!`; one shot is one trick shot however many times
it goes off.

A wand's blast sets off everything it covers, traps first and then coins
(`wands::elemental_blast`, via `things_in::<Trap>` / `things_in::<Pickup>`),
and the coins pay *the zapper* — the blast's author is the shooter. None of
those bursts is itself a blast, so nothing re-enters `elemental_blast` and a
row of them cannot chain forever.

A coin set off with no author at all — caught in somebody else's chain
reaction — is simply spent: `detonate_pickup` takes an
`Option<Entity>` and pays nobody when there is nobody to pay.

Damage traps ignore the defender's armour *die* but still subtract the
armour *plus* (`total_armor_plus`). Their bite also scales with depth in
three bands (floors 1-4, 5-8, 9-13): each band adds a point to the arrow
trap's roll and a point to the dart trap's permanent power drain. Dial:
`constants::traps` (`TRAP_DAMAGE_TIER_LAST_DEPTH` and the per-tier steps).

The `Trap`, `TrapEffect`, `TrapReveal`, `Snare` and `SnareKind` types are
defined in `components.rs`, not `traps.rs` -- see `components.md`.


DROPS -- DropCategory
---------------------

    models/src/spawn.rs

| Field       | Type           | Notes                                   |
|-------------|----------------|-----------------------------------------|
| `name`      | `&'static str` | The category, not an item name.         |
| `weight`    | `u32`          | Share of a floor's drops.               |
| `min_depth` | `u8`           | Shallowest floor it drops on.           |

Three further fields are function pointers filled in by the `category!`
macro from the table's name. Never write them by hand.

Current weights, which happen to total 1000:

| Category | Weight | Share |
|----------|--------|-------|
| scroll   | 300    | 30.0% |
| potion   | 270    | 27.0% |
| coin     | 170    | 17.0% |
| armor    |  80    |  8.0% |
| wand     |  50    |  5.0% |
| ring     |  50    |  5.0% |
| weapon   |  36    |  3.6% |
| ammo     |  28    |  2.8% |
| launcher |  16    |  1.6% |

The share column is derived, not maintained.


EFFECTS -- the marker registry
------------------------------

    models/src/effects.rs

Order is the save format: an effect's index is its bit in an `EffectSet`.
**Append only.** `EffectSet` is a `u32`, so the ceiling is 32 effects.

| # | Effect              | Meaning                                        |
|---|---------------------|------------------------------------------------|
| 0 | `FireImmune`        | Fire does nothing.                             |
| 1 | `ColdImmune`        | Cold does nothing.                             |
| 2 | `Undead`            | Draining passes through, healing nothing.      |
| 3 | `VorpalTarget`      | Any vorpal weapon slays it outright.           |
| 4 | `SeesInvisible`     | Sees hidden traps, monsters, stashed items.    |
| 5 | `SustainsStrength`  | Immune to dart-trap strength drain.            |
| 6 | `AggravatesMonsters`| Periodically wakes the floor. Passive.         |
| 7 | `ItemUser`          | Catches and wears thrown gear; reads scrolls.  |
| 8 | `FireArrow`         | Looses arrows properly (doubles their die).    |
| 9 | `FireQuarrel`       | The crossbow's half of the same bargain.       |
|10 | `SustainsArmor`     | Worn armour cannot be corroded.                |
|11 | `RustsArmor`        | Every blow it lands eats a point of the victim's armour plus (the aquator). |
|12 | `Sluggish`          | Acts one notch below its own tempo. Folded in by `conditions::tempo`, never written to `Speed`. |
|13 | `Stealthy`          | Unnoticed until `rings::STEALTH_RANGE` tiles away. |
|14 | `Regenerates`       | Mends one condition, or a point of drained power, on a roll. Passive. |
|15 | `Teleportitis`      | Jumps somewhere else on a roll. Passive. Also arms the `T` key. |

Cap components -- ceilings the dice cannot beat. Folded with `min`, not
`+`, because the strictest one wins. Not in `EFFECTS`, not bits:

    MeleeCap     most the bearer can deal in one melee blow (a bow: 1)

Modifier components -- numbers that stack across equipped gear, folded by
`equipped_total::<C>()`. Not in `EFFECTS`, not bits:

    PowerDie     adds to the attack die size
    PowerBonus   flat, added once to the damage roll
    ArmorDie     adds to the defence die size
    ArmorBonus   flat, added once to the armour roll
    ThrowBonus   flat, added once to anything thrown


PASSIVE_ABILITIES -- PassiveAbility
-----------------------------------

    models/src/abilities.rs

| Field     | Type                     | Notes                            |
|-----------|--------------------------|----------------------------------|
| `effect`  | `Grant`                  | The marker that arms it.         |
| `chance`  | `f64`                    | Probability per acting turn.     |
| `action`  | `fn(&mut World, Entity) -> bool` | Run on the bearer; reports whether it actually did anything. |
| `flavour` | `&'static str`           | Logged only for the player, and only when `action` returned `true`. |

Write `flavour` in the second person. The `bool` is what lets a passive
that often has nothing to do (a ring of regeneration on an unhurt player,
rolling every other turn) stay silent instead of narrating a non-event.

Three rows: `AggravatesMonsters` (10%), `Regenerates` (50%),
`Teleportitis` (1/85). `passive_ability_system` runs at the **tail** of
the turn schedule, after `ai` and before `visibility_system` — so a
passive that moves its bearer lands the jump at the top of the bearer's
next turn, and the player acts from the new tile before anything on the
floor moves again.


ON_HIT_ABILITIES -- OnHitAbility
--------------------------------

    models/src/abilities.rs

The other moment an effect can act on its own: a blow that connected.
`combat::resolve_attack` fires the table for every hit that dealt damage
and never learns what is in it.

| Field         | Type                             | Notes                        |
|---------------|----------------------------------|------------------------------|
| `effect`      | `Grant`                          | The marker on the *attacker*. |
| `on_glancing` | `bool`                           | Whether a glancing scrape counts. |
| `on_lethal`   | `bool`                           | Whether the killing blow counts. |
| `action`      | `fn(&mut World, Entity, Entity)` | Run as `(attacker, target)`. |

Two rows:

| Effect | Glancing? | Lethal? | Does |
|---|---|---|---|
| `ConfusingTouch` | no | no | `items::discharge_confusing_touch` — a scroll of monster confusion's charm, spent passing itself on. Needs skin, and there is no point charming a corpse. |
| `RustsArmor` | yes | yes | `equipment::corrode_armor` — the aquator. Acid does not care that the armour turned the blow; it landed on the armour. |

`ConfusingTouch` is named here by a `Grant` handle but is deliberately
*not* in `EFFECTS`: it is a condition with its own save field
(`saveload`), not a cancellable creature property. `Grant::probe` works
either way — the registry is only about save bits and cancellation.


Enchantment
-----------

Rolled by `enchant_equipment` for every weapon, armour, launcher and ring
that arrives as a floor drop. Never for a `spawn_named` spawn. The odds
and the bonus ranges are `models/src/constants.rs` → `loot` (see
`reference/constants.md`).

| Quality     | Odds | Bonus            |
|-------------|------|------------------|
| Normal      | 25%  | +0               |
| Exceptional | 10%  | +1 .. +3         |
| Cursed      | 65%  | -5 .. +5, plus a `Curse` tag |

A cursed item can roll better than a clean one; it simply cannot be taken
off once equipped, short of a scroll of remove curse (which destroys it)
or the matching scroll of enchantment (which lifts the `Curse` tag and
mends any minus to `+0` -- see `constants::scrolls::ENCHANT_BONUS`).

The bonus lands on whichever roll the item feeds, read off the item
itself: a thing with a `PowerDie` gets `PowerBonus`, a thing with an
`ArmorDie` gets `ArmorBonus`, a `Launcher` gets `ThrowBonus`. Something
that is two of those would get both. Never on the die size.


Identification
--------------

    models/src/identify.rs

Four categories arrive unidentified: potions, scrolls, wands, rings. Each
has a pool of 20 cosmetic appearances, shuffled against the catalog once
per run from the seeded RNG and then stored in the save.

| Category | Types | Pool | Headroom |
|----------|-------|------|----------|
| Potions  | 15    | 20   | 5        |
| Scrolls  | 15    | 20   | 5        |
| Wands    | 14    | 20   | 6        |
| Rings    | 12    | 20   | 8        |

Exceeding a pool leaves the extra types with no appearance, permanently
generic. Guarded by `every_identifiable_type_gets_an_appearance` in
`models/tests/content.rs`.

Knowledge is stored against the effect, not the entity: identifying one
bubbly potion identifies every bubbly potion, forever.


The colour palette
------------------

The save file packs a colour into one byte against this fixed list.
Anything not on it draws correctly but reloads as `White`.

    Black       DarkGrey    Grey        White
    Red         DarkRed     Green       DarkGreen
    Yellow      DarkYellow  Blue        DarkBlue
    Magenta     DarkMagenta Cyan        DarkCyan


Dungeon constants
-----------------

    MAP_WIDTH               80
    MAP_HEIGHT              22
    FINAL_DEPTH             13      the floor holding the relic
    DUNGEON_LORD_PATIENCE  260      turns on one floor before eviction
    STACK_LIMIT             13      most of one item per pack slot
    PACK_CAPACITY             9      most slots a pack will hold at once
    THROW_RANGE              7      how far you can hurl a thing
    Speed::COST              2      energy one action costs

These, and every other balance number, are defined and explained in
`models/src/constants.rs`. See `reference/constants.md` for the tour.


See also
--------

  spawn-api.md                  the functions that read these tables
  cli-and-env.md                flags and environment variables
  input-and-turn-loop.md        how confusion, snares, etc. play out at the keyboard
  ../how-to/add-an-item.md      how to add a row to one of these
