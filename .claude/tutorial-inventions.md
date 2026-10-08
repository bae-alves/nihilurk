# Tutorial inventions

Never commit these into the game. The tutorials and how-tos use them as worked examples. A reader should want to build them, and a reader cannot if they already exist. A reader who finds one in `-content` learns that the lesson is a lie.

Before you add content, grep this list. Before you write a new example, pick a name that the game does not have, and add it here.

Checked 2026-10-05 against `models/`, `engine/` and `strings/`: none of these exist.

| Invention | Kind | Where it is used |
|---|---|---|
| quarterstaff | weapon row | `tutorial/add-your-first-item.md`, `README.md` |
| brigandine | armour row | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md`, `README.md` |
| copper coin | coin row | `tutorial/add-your-first-item.md` |
| bone | treat row | `tutorial/add-your-first-item.md` |
| ring of fire resistance, `RingEffect::FireResistance` | ring | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md` |
| sling, sling stone, `FireStone` (id `fire_stone`) | launcher, ammo, effect | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md`, `how-to/add-an-effect.md` |
| potion of levitation, `PotionEffect::Levitation` | potion | `tutorial/add-your-first-item.md`, `how-to/add-an-item.md` |
| scroll of protect armor, `ScrollEffect::ProtectArmor` | scroll | `tutorial/add-your-first-item.md` |
| wand of sleep, `WandEffect::Sleep` | wand | `tutorial/add-your-first-item.md` |
| basilisk | monster row | `tutorial/add-your-first-monster.md`, `how-to/add-a-monster.md`, `how-to/add-an-effect.md`, `reference/cli-and-env.md`, `doc/nihilurk.6` |
| Ice Bolt, `SpellEffect::IceBolt`, `ice_bolt` | spell | `tutorial/add-your-first-spell.md`, `how-to/add-a-spell.md` |
| ninja, `Body::Ninja`, `wear_ninja`, `NINJA_GRANTS` | body | `tutorial/add-your-first-body.md` |
| wraithkin, `Body::Wraithkin`, `wear_wraithkin` | body | `how-to/add-a-body.md` |
| Cower, `MovementType::Cower`, `COWARD`, `BOLT_WHEN_CLOSE` | tactic, rule set, rule | `tutorial/give-a-monster-a-mind.md` |

## How they were checked

On 2026-10-05 I ran every tutorial in a scratch copy of the tree (its own `CARGO_TARGET_DIR`, deleted after). All the inventions compiled, and the full suite passed except four tests that assume an empty starting `Spellset`, which the spell tutorial's step 4 breaks on purpose and step 7 reverts. Nothing was added to the real tree.

Internal control for bae and Claude, filled by hand whenever either of us invents a name. No script reads it, and none should.
