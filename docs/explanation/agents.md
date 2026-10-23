How monsters think
==================

    Audience       Anyone deciding how a creature should behave, or
                   changing what a monster or a Helper does with its
                   turn.
    Prerequisites  None.
    This is        The reasoning first, then the tables to look
                   things up in. Describes `models/src/mind/agents.rs`
                   and the parts of `models/src/mind/ai.rs` that call
                   it; where the tables and the source disagree, the
                   source is right.

Every mob in nihilurk is a reflex agent. Each turn it is handed what is true right now, it picks one thing to do from an ordered list of rules, and it forgets all of it. There is no memory, no plan, no state machine to be in the middle of. This page is why.


No memory
---------

A monster that remembers has somewhere for a bug to live. "It saw you two rooms ago and is still coming" needs a place to keep the sighting, a rule for when to drop it, and a save field for it, and every one of those is a way for a monster to act on something the player can no longer see or check. A turn built from a fresh percept can only ever act on the floor as it is.

It also makes the rules testable without a world. A rule is a plain function of the `Percept`, so a test builds one by hand, runs `think`, and reads the answer. Nothing has to be spawned, walked into place or aged. The randomness a rule needs (which spell to try, which way to stagger) is rolled into the percept for the same reason: the rule stays a function of what it is handed.

The one thing that looks like memory is `Aggravated`, and it is not the monster's. It is a fact laid on the monster from outside, by a scroll or a ring, the way `Asleep` is.


The player's view is the gate
-----------------------------

A mob on a tile the player cannot see does nothing. Sight in nihilurk is symmetrical (the player sees the room, the room sees the player), so a monster out of view has, as far as the game is concerned, nothing to perceive. The floor stays quiet out of sight, and nothing happens off-screen that the player could not have watched.

Three things break that on purpose. An aggravated monster heard a shriek, and a shriek carries through walls, so it walks toward the noise. The player's Helper walks back to the player, because a companion stranded around a corner is no companion. And a charmed ally that is not the Helper counts as in view wherever it stands and sees every monster on the floor, because a charmed creature that switched off in the next room would be no use to anyone. The first two are single moves with no percept behind them, and both hand back to their rule set the moment they are in view: an aggravated venus flytrap walks in from the next room, then lies in wait like a flytrap once you can see it.

| Mob | Off-view action |
|---|---|
| carries `Aggravated { tx, ty }` | one step straight toward `(tx, ty)`, never leashed; waits once there |
| is the player's `Helper` | the `heel` rule, never leashed |

Both switch to their rule set the moment they stand in view. "The player's view" is the view with their eyes open: a blinded player is still seen by the room they stand in.

That is also why aggravation is a component. As a `MovementType` it would overwrite the tactic it was laid on, and an aggravated flytrap would forget it was a flytrap. As a state on top of the tactic, the tactic is still there to go back to.


First match wins
----------------

A rule set is an ordered list, and the first rule that fires takes the turn. Order is the whole design: `CAST` before `SHOOT` before `STRIKE` before `CLOSE_IN` says "magic if you can, a bow if you can, otherwise go to melee", and that sentence is the set. There are no weights to tune and no scores to compare, so reading a set tells you exactly what the creature does, and moving one line changes it.

The chaser's loop is deliberately dumb about its spells: it picks one at random and fires it if it can, and if it cannot it does not try another. A dragon with one spell breathes every time a foe is in range and the Ma is there. A creature with three spells is unpredictable in the way a player can plan around, rather than always casting its best one.


Allies look before they cast
----------------------------

A wild monster does not care whom its fireball catches, its kin included. An ally does: every spell an ally casts is checked against the player's tile first, and a foe standing too close to the player is passed over for one further off, or for a bite. That check lives in the rule (`CAST` asks `ally`), not in the spell, because the spell is the same mechanic whoever casts it. What differs is whose side the caster is on.

The footprint comes off the spell's own row: its blast and spread, else the line it flies, else the tile it lands on. A new spell row brings its footprint with it, so an ally never casts one blind. The check does not follow chain reactions: a fireball that sets off a trap or a coin can still burst onto the player.


What `ai.rs` keeps
------------------

The clock (energy, rounds, the two passes a fast monster gets), the gates (dead, asleep, stone, a paralysed turn lost, a door the player just stepped onto) and the hands (carrying an `Action` into the world). It never decides anything. A new behaviour is a new rule or a new set in `agents.rs`, and `ai.rs` does not change.


The turn, in order
------------------

    models/src/mind/ai.rs   step_one_mob

  1. **Gates.** Dead (0 HP), `Asleep`, `Petrified` or out of energy: no turn. `Paralyzed` loses the turn on a `PARALYSIS_LOST_TURN_CHANCE` roll, energy spent. A `Faction::Monster` mob that has noticed the player loses the turn while the player has just stepped onto a door (`door_ward`).
  2. **Percept.** `perceive` builds it (below).
  3. **Rule set.** `rule_set_for(movement_type, helper, ally)`.
  4. **Decision.** `think(&percept, set)`:
       * off the player's view: the off-view rule (below), and the rule set is never asked;
       * in view: the first rule that returns `Some`, or `Action::Wait`.
  5. **Action.** `act` carries it out. A `Strike` is a step onto the foe's tile, so both pass the same checks (`mob_can_enter`): on the map, walkable (water only for a swimmer), a legal diagonal, and the room leash when `leashed(&percept, set)` is true and the step is not a blow. A `Phasing` mob skips those checks. A step onto a foe queues a blow on `AttackQueue`. A step onto a creature it would not fight slides to one of the two diagonals beside it, picked by a roll (`slide_past`), and stands if neither is open. A pinned mob never steps.


`Percept`
---------

| Field | Type | What it holds |
|---|---|---|
| `at` | `Position` | Where the mob stands. |
| `in_view` | `bool` | The mob's tile is in the player's view; always true for a charmed ally that is not the Helper. |
| `player_at` | `Position` | Where the player stands. |
| `noticed` | `bool` | In view, and within `STEALTH_RANGE` if the player is `Stealthy`. |
| `pinned` | `bool` | `Pinned`, `Rooted` or `Clamped`: strikes, but never steps or shoots. |
| `swims` | `bool` | Deep water is floor to it. |
| `phasing` | `bool` | `Phasing`: it walks through walls and water, and no corner holds it. |
| `launcher` | `bool` | A launcher is drawn in its hand. |
| `spellset` | `Vec<&'static SpellDef>` | Each spell in its `Spellset` that its `Magic` can pay for. |
| `roll` | `u32` | A die rolled for this turn off the seed's `GameRng`, so a seeded run replays it. |
| `helper` | `bool` | It is the player's `Helper`. |
| `ally` | `bool` | Its `Faction` is `Ally`: its spells never go where they would catch the player. |
| `aggravated` | `Option<Position>` | Where an `Aggravated` mob is heading. |
| `foes` | `Vec<Sighting>` | Everything it would come to blows with (`ai::hostile`) on a tile the player can see (anywhere, for a charmed ally that is not the Helper) and not `Hidden`, nearest first, ties by tile. The player is in it only once `noticed`. |
| `map` | `&Map` | The floor. |

`Sighting` is `{ who: Entity, at: Position, is_player: bool }`.


`Action`
--------

| Variant | Carried out by |
|---|---|
| `Strike(Entity)` | A step onto the foe's tile, which queues a blow. |
| `Shoot(Entity)` | `items::monster_ranged_attack`. |
| `Cast(&'static SpellDef, Position)` | `items::pay_for_spell`, then `items::cast_spell(spell, mob, at, 1)`. |
| `Step(dx, dy)` | One step; into a foe, a blow. |
| `Wait` | Nothing. The turn is not spent. |


The rules
---------

Each is a `const Rule` wrapping a plain function of the percept.

| Rule | Fires when | Action |
|---|---|---|
| `CAST` | The spellset is not empty. It picks `spellset[roll % len]` and fires it at the nearest foe in the spell's `SpellDef::range`. Only a row with `line` set needs a clear line; the rest are aimed at the foe's tile. An ally passes over any foe whose footprint (`agents::reaches`) holds the player. Only an `Attack` spell with a range above 0 is ever fired. Never at a foe across a corner (`across_corner`). | `Cast` |
| `SHOOT` | A launcher is drawn, not pinned, a foe within `MONSTER_SHOT_RANGE` with a clear line, and not across a corner (`across_corner`: next to it, but past a doorway or room corner it could not step across). | `Shoot` the nearest such foe |
| `STRIKE` | A foe on one of the eight tiles around it, bar one across a corner (`across_corner`). | `Strike` it, the player before anyone else |
| `CLOSE_IN` | Any foe: the player once noticed, or an ally of theirs. | `Step` along the shortest walk to the nearest |
| `HEEL` | More than one tile from the player. | `Step` along the shortest walk to a tile next to them |
| `FLEE` | The player is among the foes. | `Step` straight away from them |
| `STAGGER` | Always. | `Step` north, south, east or west by `roll` |

A clear line (`agents::clear_line`) is one no wall breaks; bodies in it do not count.

A spell's footprint (`agents::reaches`) is read off its row. A row with a `blast` covers `items::blast_cells` around its target at that radius plus its `spread` (line of sight from the centre). Otherwise a row with `line` covers its line up to the target, stopping at the first wall. Otherwise it covers the target tile alone. Pinned by `a_helper_keeps_the_player_out_of_a_rows_blast_and_its_spread` and `a_helper_keeps_the_player_off_a_rows_line` in `agents.rs`.

The shortest walk is `autoexplore::first_step` over tiles the mob can stand on (any tile at all for a `Phasing` mob), biased toward the goal. A mob knows the floor it stands on, so it is not limited to tiles the player has seen. `across_corner` never holds a `Phasing` mob either.


The sets
--------

| Set | Picked when | `leashed` | Rules, in order |
|---|---|---|---|
| `STILL` | `MovementType::Static` | no | (none) |
| `CHASER` | `MovementType::Chase` (and the retired `Aggravated`) | yes | `CAST`, `SHOOT`, `STRIKE`, `CLOSE_IN` |
| `ALLY` | `Chase` on a `Faction::Ally` that is not a `Helper` (a charmed monster) | no | `CAST`, `SHOOT`, `STRIKE`, `CLOSE_IN` |
| `AMBUSHER` | `MovementType::Ambush` | no | `STRIKE` |
| `FLEER` | `MovementType::Flee` | no | `FLEE` |
| `STAGGERER` | `MovementType::Confused` | no | `STAGGER` |
| `HELPER` | the mob is a `Helper`, whatever its variant | no | `CAST`, `SHOOT`, `STRIKE`, `CLOSE_IN`, `HEEL` |

The room leash: from a `Room` tile, a leashed mob will not step onto a `Passage` or a `Door`. The leash belongs to the tile it stands on, so from a corridor it crosses a door freely.

A `CHASER`'s turn reads as: pick a spell at random, fire it if it can, else shoot if it can, else go to melee. A wild monster does not care whom its spell catches. A `CHASER` closes in on the nearest foe, so an ally of the player standing nearer than the player draws it away from them; `STRIKE` still hits the player first when both are alongside. A charmed monster's `ALLY` set is the same turn aimed at the monsters, and it does nothing when none is in view. The `HELPER`'s is the same turn aimed at the monsters, with the player never in the blast, and a walk back to the player when there is nothing to fight.


Where the pieces live
---------------------

| Piece | File |
|---|---|
| `Percept`, `Action`, `Rule`, `RuleSet`, the rules, the sets, `think`, `rule_set_for` | `models/src/mind/agents.rs` |
| `perceive`, `act`, `slide_past`, `mob_can_enter`, the gates, energy and rounds | `models/src/mind/ai.rs` |
| `MonsterDef::casts` (a species' spells and Ma) | `models/src/creatures/monsters.rs` |
| `can_afford_spell`, `pay_for_spell` | `models/src/items/spells.rs` |
| `Aggravated` | `models/src/meta/components/` |
| `blast_cells` | `models/src/items/wands.rs` |


See also
--------

  ../how-to/add-a-rule-set.md       a new way to think
  ../how-to/add-a-rule.md           a new reflex
  data-driven-content.md            the same idea for content tables
  adr-0007-aggravation-is-a-component.md  why aggravation sits on top of the tactic
