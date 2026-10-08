ADR 0008: the player's step is a queued intent
==============================================

    Status         Accepted
    Audience       Anyone adding a verb the player can do, or moving a
                   rule in or out of `engine/src/update.rs`.
    Supersedes     --
    Related        adr-0005-aim-resolves-before-the-mobs-move.md

The player's step, blow, lunge and struggle, and taking the stairs, dropping, a willed teleport, a charge and a reach attack, are decided when the key is handled and applied by the first step of the schedule. The use, throw and spell queues already worked this way.


Context
-------

A step used to happen in the input handler, before the schedule ran. Every other intent was queued and drained by a schedule step, so the player's own verbs were the one place a rule lived outside the turn. A marker such as `EntityMoved` was set by the input layer and cleared by `trap_system`, and nothing but a comment tied the two.


Decision
--------

**Plan at the key, apply in the schedule.**

  1. `models::plan_step` reads the world and returns a `StepPlan`. It changes nothing except the confusion roll, which has to happen at the key so the random draws stay in their old order.
  2. `models::queue_step` plans, queues a `PlayerAction::Step` unless the plan was a refusal, and returns whether the turn is spent. A bump into a wall is a refusal that spends nothing, so the monsters do not move. The other verbs follow the same shape: `queue_stairs`, `queue_drop`, `queue_willed_teleport`, `queue_charge` and `queue_reach_attack` check what would refuse, log it and return false, or queue the action.
  3. `player_action_system` is the schedule's first step. It drains `PlayerActionQueue` and applies each plan. Everything else in the turn runs after it, so the player still acts before `ai`.

The plan is made against the world as it stands when the key is handled. Nothing runs between that moment and the schedule, so the plan is still true when it is applied.


Consequences
------------

Accepted costs:

  * **A fifth queue and a seventeenth step.** Every page that states either number changes.
  * **A step is no longer visible the instant the handler returns.** Code that reads the world right after `queue_step` sees the old position. Callers run the schedule first, and tests call `player_action_system`.
  * **Two places describe one decision tree.** `plan_step` and `apply_step` both name the same variants.

Benefits realised:

  * A bot can drive the real turn by queueing actions, which `models/tests/soak.rs` does.
  * `EntityMoved` is set and cleared inside `models`.
  * The rules of a step live in `models`, where they are tested without a terminal.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **A verb cannot be planned without running it.** Then the plan would need to run the rule to know whether it spends the turn.
  * **Something must run between the key and the schedule.** The plan would no longer be true when applied.


See also
--------

  adr-0005-aim-resolves-before-the-mobs-move.md   the same ordering, for aimed use and throw
  ../reference/input-and-turn-loop.md             the step, in order, and the schedule
  ../reference/components.md                      `PlayerActionQueue`
