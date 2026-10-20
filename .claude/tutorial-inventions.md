# Tutorial inventions

Never commit these into the game. The tutorials and how-tos use them as worked examples. A reader should want to build them, and a reader cannot if they already exist. A reader who finds one in `-content` learns that the lesson is a lie.

Before you add content, grep this list. Before you write a new example, pick a name that the game does not have, and add it here.

Checked 2026-10-20 against `models/`, `engine/` and `strings/`: none of these exist.

| Invention | Kind | Where it is used |
|---|---|---|
| quarterstaff | weapon row | `tutorial/add-your-first-item.md`, `README.md` |
| javelin | weapon row | `how-to/add-an-item.md` |
| brigandine | armour row | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md`, `README.md` |
| copper coin | coin row | `tutorial/add-your-first-item.md` |
| electrum coin | coin row | `how-to/add-an-item.md` |
| bone, `BONE` | treat row | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md` |
| bolt, `BOLT` | ammo row | `how-to/add-an-item.md` |
| ring of fire resistance | ring row | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md` |
| ring of poison resistance, `PoisonImmune` (id `poison_immune`) | ring row, effect | `how-to/add-an-effect.md` |
| sling, sling stone, `FireStone` (id `fire_stone`) | launcher, ammo, effect | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md`, `how-to/add-an-effect.md` |
| potion of levitation, `potions::levitation` | potion | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md` |
| scroll of protect armor, `scrolls::protect_armor` | scroll | `tutorial/add-your-first-item.md` |
| scroll of genocide, `scrolls::genocide` | scroll | `how-to/add-an-item.md` |
| wand of sleep, `wands::sleep`, `wands::put_to_sleep` | wand | `tutorial/add-your-first-item.md` |
| food, ration, `FoodDef`, `Food`, `FOODS` | item category | `how-to/add-an-item-category.md` |
| pit trap | trap row | `how-to/add-a-trap.md`; `content::a_traps_label_is_its_row` asserts it is absent |
| basilisk | monster row | `tutorial/add-your-first-monster.md`, `how-to/add-a-monster.md`, `how-to/add-an-effect.md`, `reference/cli-and-env.md`, `doc/nihilurk.6` |
| Ice Bolt, `ice_bolt` | spell | `tutorial/add-your-first-spell.md`, `how-to/add-a-spell.md` |
| ninja, `NINJA`, `wear_ninja`, `NINJA_GRANTS` | body | `tutorial/add-your-first-body.md` |
| wraithkin, `WRAITHKIN`, `wear_wraithkin` | body | `how-to/add-a-body.md` |
| Cower, `MovementType::Cower`, `COWARD`, `BOLT_WHEN_CLOSE` | tactic, rule set, rule | `tutorial/give-a-monster-a-mind.md` |
| Skirmish, `SKIRMISHER`, `KEEP_DISTANCE` | tactic, rule set, rule | `how-to/add-a-rule.md`, `how-to/add-a-rule-set.md` |
| `SightBonus` | modifier | `how-to/add-an-effect.md` |

## How they were checked

On 2026-10-20 every tutorial and every how-to recipe that changes code was followed in a throwaway worktree, removed after. All the inventions compiled and the suites they touch passed, with two expected exceptions: the spell tutorial's step 4 breaks five tests that count the starting `Spellset` until step 7 reverts it, and the pit trap fails the content test that keeps it out of the game. Nothing was added to the real tree.

Internal control for bae and Claude, filled by hand whenever either of us invents a name. No script reads it, and none should.
