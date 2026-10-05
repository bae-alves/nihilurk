Reference: agents
=================

    Audience       Anyone changing what a monster or a Helper does with
                   its turn. Look things up here; do not read it end to
                   end.
    Prerequisites  None.
    Status         Describes `models/src/agents.rs` and the parts of
                   `models/src/ai.rs` that call it. If this page and the
                   source disagree, the source is right and this page is
                   a bug.

Every mob decides its turn the same way: `ai` builds a `Percept`, `agents::think` hands it to the mob's `RuleSet`, and the first `Rule` that fires names one `Action`, which `ai` carries out. Nothing is remembered between turns. Why it is built this way is `../explanation/agents.md`.


The turn, in order
------------------

    models/src/ai.rs   step_one_mob

  1. **Gates.** Dead (0 HP), `Asleep`, `Petrified` or out of energy: no turn.
  2. **Percept.** `perceive` builds it (below).
  3. **Rule set.** `rule_set_for(movement_type, helper)`.
  4. **Decision.** `think(&percept, set)`:
       * off the player's view: the off-view rule (below), and the rule set is never asked;
       * in view: the first rule that returns `Some`, or `Action::Wait`.
  5. **Action.** `act` carries it out. A `Strike` is a step onto the foe's tile, so both pass the same checks (`mob_can_enter`): on the map, walkable (water only for a swimmer), a legal diagonal, and the room leash when `leashed(&percept, set)` is true. A step onto a foe queues a blow on `AttackQueue`; onto anyone else, the mob stands. A pinned mob never steps.


Off the player's view
---------------------

A mob on a tile the player cannot see does nothing, with two exceptions:

| Mob | Off-view action |
|---|---|
| carries `Aggravated { tx, ty }` | one step straight toward `(tx, ty)`, never leashed; waits once there |
| is the player's `Helper` | the `heel` rule, never leashed |

Both switch to their rule set the moment they stand in view. "The player's view" is the view with their eyes open: a blinded player is still seen by the room they stand in.


`Percept`
---------

| Field | Type | What it holds |
|---|---|---|
| `at` | `Position` | Where the mob stands. |
| `in_view` | `bool` | The mob's tile is in the player's view. |
| `player_at` | `Position` | Where the player stands. |
| `noticed` | `bool` | In view, and within `STEALTH_RANGE` if the player is `Stealthy`. |
| `pinned` | `bool` | `Pinned`, `Rooted` or `Clamped`: strikes, but never steps or shoots. |
| `swims` | `bool` | Deep water is floor to it. |
| `launcher` | `bool` | A launcher is drawn in its hand. |
| `spellset` | `Vec<SpellEffect>` | Each spell in its `Spellset` that its `Magic` can pay for. |
| `roll` | `u32` | A die rolled for this turn (`getrandom`, not the seed's `GameRng`). |
| `helper` | `bool` | It is the player's `Helper`. |
| `ally` | `bool` | Its `Faction` is `Ally`: its spells never go where they would catch the player. |
| `aggravated` | `Option<Position>` | Where an `Aggravated` mob is heading. |
| `foes` | `Vec<Sighting>` | Everything it would come to blows with (`ai::hostile`) on a tile the player can see and not `Hidden`, nearest first, ties by tile. The player is in it only once `noticed`. |
| `map` | `&Map` | The floor. |

`Sighting` is `{ who: Entity, at: Position, is_player: bool }`.


`Action`
--------

| Variant | Carried out by |
|---|---|
| `Strike(Entity)` | A step onto the foe's tile, which queues a blow. |
| `Shoot(Entity)` | `items::monster_ranged_attack`. |
| `Cast(SpellEffect, Position)` | `items::apply_spell_effect(mob, at, spell, 1)`. Free: no Magic. |
| `Step(dx, dy)` | One step; into a foe, a blow. |
| `Wait` | Nothing. The turn is not spent. |


The rules
---------

Each is a `const Rule` wrapping a plain function of the percept.

| Rule | Fires when | Action |
|---|---|---|
| `CAST` | The spellset is not empty. It picks `spellset[roll % len]` and fires it at the nearest foe in the spell's `SpellDef::range` with a clear line. An ally passes over any foe whose footprint reaches the player. Only an `Attack` spell with a range above 0 is ever fired. | `Cast` |
| `SHOOT` | A launcher is drawn, not pinned, a foe within `MONSTER_SHOT_RANGE` with a clear line. | `Shoot` the nearest such foe |
| `STRIKE` | A foe on one of the eight tiles around it. | `Strike` it, the player before anyone else |
| `HUNT` | The player is among the foes (noticed). | `Step` along the shortest walk to them |
| `CLOSE_IN` | Any foe. | `Step` along the shortest walk to the nearest |
| `HEEL` | More than one tile from the player. | `Step` along the shortest walk to a tile next to them |
| `FLEE` | The player is among the foes. | `Step` straight away from them |
| `STAGGER` | Always. | `Step` north, south, east or west by `roll` |

A clear line (`agents::clear_line`) is one no wall breaks; bodies in it do not count.

A spell's footprint (`agents::reaches`): a Fireball covers `items::blast_cells` around its target (`BLAST_RADIUS`, line of sight from the centre); a Thunderbolt covers its target tile. A spell whose footprint is not listed there is assumed to reach everyone, so an ally never casts it.

The shortest walk is `autoexplore::first_step` over tiles the mob can stand on, biased toward the goal. A mob knows the floor it stands on, so it is not limited to tiles the player has seen.


The sets
--------

| Set | Picked when | `leashed` | Rules, in order |
|---|---|---|---|
| `STILL` | `MovementType::Static` | no | (none) |
| `CHASER` | `MovementType::Chase` (and the retired `Aggravated`) | yes | `CAST`, `SHOOT`, `STRIKE`, `HUNT` |
| `AMBUSHER` | `MovementType::Ambush` | no | `STRIKE` |
| `FLEER` | `MovementType::Flee` | no | `FLEE` |
| `STAGGERER` | `MovementType::Confused` | no | `STAGGER` |
| `HELPER` | the mob is a `Helper`, whatever its variant | no | `CAST`, `SHOOT`, `STRIKE`, `CLOSE_IN`, `HEEL` |

The room leash: from a `Room` tile, a leashed mob will not step onto a `Passage` or a `Door`. The leash belongs to the tile it stands on, so from a corridor it crosses a door freely.

A `CHASER`'s turn reads as: pick a spell at random, fire it if it can, else shoot if it can, else go to melee. A wild monster does not care whom its spell catches. The `HELPER`'s is the same turn aimed at the monsters, with the player never in the blast, and a walk back to the player when there is nothing to fight.


Where the pieces live
---------------------

| Piece | File |
|---|---|
| `Percept`, `Action`, `Rule`, `RuleSet`, the rules, the sets, `think`, `rule_set_for` | `models/src/agents.rs` |
| `perceive`, `act`, the gates, energy and rounds | `models/src/ai.rs` |
| `MonsterDef::casts` (a species' spells and Ma) | `models/src/monsters.rs` |
| `can_afford_spell`, `pay_for_spell` | `models/src/items/spells.rs` |
| `Aggravated` | `models/src/components.rs` |
| `blast_cells` | `models/src/items/wands.rs` |


See also
--------

  ../explanation/agents.md          why a reflex agent, and why the view
  ../how-to/add-a-rule-set.md       a new way to think
  ../how-to/add-a-rule.md           a new reflex
  ../tutorial/give-a-monster-a-mind.md   one set, start to finish
