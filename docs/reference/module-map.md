Reference: the models module map
================================

    Audience       Anyone who has a name in hand -- a type, a system, a
                   table -- and needs the file it lives in.
    Prerequisites  None.
    Status         Describes `models/src/` as it is in the source. If
                   this page and the source disagree, the source is
                   right and this page is a bug.

The dry facts: which folder and file holds what. For how the pieces fit together, read `../explanation/ecs-in-nihilurk.md` and `../explanation/data-driven-content.md`.

`models/src/` is seven folders. Every module is re-exported at the crate root, so `models::map::Map` and `models::Map` are the same type, and moving a file between folders never changes a call site. The folders are for the reader.


world/ -- the place
-------------------

    world/map.rs              the terrain: `Map`, `TileType`, and the questions every system asks of it
    world/map/generate.rs     carving a floor: rooms, corridors, stairs
    world/map/levels.rs       moving between floors, and the two ways a run ends
    world/map/population.rs   what stands on a floor once it is carved
    world/map/special.rs      special levels that are not the 3x3 of rooms
    world/map/overlays.rs     blood, corpses and smoke
    world/map/streams.rs      the three random streams a run draws from
    world/rect.rs             a room's footprint
    world/visibility.rs       what the player's eyes report each turn
    world/magicmap.rs         the scroll of magic mapping's reveal
    world/traps.rs            the trap table and what each trap does
    world/ice.rs              the cube a cold kill leaves behind
    world/spawn.rs            where content meets the dungeon (`spawn_named`)


creatures/ -- who lives there
-----------------------------

    creatures/monsters.rs     the bestiary (`BESTIARY`, `SUMMONS`) and how a row spawns
    creatures/body.rs         what the player *is*: the playable bodies
    creatures/abilities.rs    effects that act on their own, one row each
    creatures/conditions.rs   what a creature is afflicted with, and lifting it
    creatures/spirits.rs      `Faction::Spirits`, neutral until crossed
    creatures/companion.rs    the boon companion
    creatures/pack.rs         the pack screen and its menus


mind/ -- how anything decides
-----------------------------

    mind/ai.rs                what every monster does with its turn
    mind/agents.rs            what a monster perceives, and the rule sets (`rule_set_for`)
    mind/autoexplore.rs       the player's `o`
    mind/autofight.rs         the player's `Tab`


items/ -- what the player carries
---------------------------------

    items/catalog.rs          every item, one row each: the file you edit to add content
    items/mod.rs              the item system: drains the use queue, hands off by kind
    items/potions.rs          quaffing
    items/scrolls.rs          reading
    items/wands.rs            zapping, and the bolt and blast machinery
    items/throwing.rs         hurling anything at anything
    items/spells.rs           the active-spell system
    items/rings.rs            the rings whose effect is a verb
    items/runes.rs            invoking a rune
    items/decks.rs            decks of cards
    items/pickups.rs          coins, and what stepping on one does
    items/theft.rs            what a thieving monster does with what it takes
    items/equipment.rs        wearing and wielding
    items/identify.rs         identification, equipment only


combat/ -- the blow and the vocabulary
--------------------------------------

    combat/mod.rs             the systems, and `entity_name`
    combat/melee.rs           the opposed roll: `resolve_attack`, `melee_attack`, `mean_blow`
    combat/reach.rs           lunge, whirl, reach weapons
    combat/report.rs          the log lines and sparks a landed blow makes
    combat/death.rs           what a death sets off
    combat/helpers.rs         plumbing; `apply_hit` and `Hit` are the one place mitigation is decided
    combat/effects/           the effect vocabulary, below

`combat/effects/` is where a new effect goes (`../how-to/add-an-effect.md`):

    markers.rs                the yes/no effects
    status.rs                 confused, blind, paralyzed, asleep, pinned, ...
    modifiers.rs              the numeric effects, `Loadout`, `equipped_total`
    registry.rs               `Grant`, and the `EFFECTS` table a save reads
    ledger.rs                 `Lifetime`, `Held`, `lend`, `revoke`
    upkeep.rs                 `tick_effects`, `hold`, `stone_chip`


player/ -- the player's turn
----------------------------

    player/mod.rs             a step, planned at the key and applied by the schedule
    player/fastmove.rs        runs and travel
    player/keymap.rs          key remapping
    player/conduct.rs         the things a run can go without


meta/ -- what the rest hangs on
-------------------------------

    meta/components/          the ECS vocabulary, nouns only; split by theme
        identity.rs  creatures.rs  items.rs  conditions.rs  events.rs  ui.rs  run.rs
    meta/constants/           every tuning knob, in six files by what they tune
        world_tuning.rs  creature_tuning.rs  item_tuning.rs  combat_tuning.rs
        player_tuning.rs  meta_tuning.rs
    meta/saveload.rs          the save file
    meta/schedule.rs          the turn, in order
    meta/leaderboard.rs       the leaderboards
    meta/score.rs             the number on the HUD
    meta/state.rs             is the run going, and how did it end
    meta/hud.rs               presentation helpers the renderer and input share
    meta/pride.rs             the flags
    meta/color.rs             the 16-colour palette
    meta/particles.rs         throwaway visual feedback
    meta/particle_core.rs     the arithmetic half of the particle layer
    meta/shake.rs             screen shake


Finding a name
--------------

| You have                                  | Look in                                   |
|-------------------------------------------|-------------------------------------------|
| a `Component`, `Resource` or `Event`      | `meta/components/`                        |
| a number you want to turn                 | `meta/constants/`                         |
| a marker or modifier effect               | `combat/effects/`                         |
| a row in a content table                  | `items/catalog.rs`, `creatures/monsters.rs`, `world/traps.rs` |
| a system, and when it runs                | `meta/schedule.rs`                        |
| a test for a rule                         | `models/tests/it/<folder>/`, same folder names as above |

Each folder's `mod.rs` opens with a header that lists its modules, and each file opens with one that says what it owns.
