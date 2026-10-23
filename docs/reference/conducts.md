Reference: conducts
===================

    Audience       Engine developer adding a conduct, or anyone reading
                   a run dump or the leaderboard.
    Prerequisites  Rust, and a passing familiarity with bevy_ecs.
    Status         As the source is. If this page and the source
                   disagree, the source is right and this page is a bug.

A conduct is something a run can go without. A run that wins is paid `PERCENT_PER_CONDUCT` percent of its score for each conduct it kept, added on top, and the percentages add: every conduct kept is `PERCENT_PER_CONDUCT` times the length of `Conduct::ALL`. A run that dies is paid nothing, and its dump lists no conducts.

Everything here lives in `models/src/player/conduct.rs` and is re-exported as `models::conduct`, bar `Conduct` and `Conducts`, which are at the crate root.


The conducts
------------

Each conduct breaks on one act, at one place in the code. Every site below is player-only: either it calls `player_broke`, which checks, or it can only ever be the player's doing. Monsters share the code and have no conducts.

| Conduct     | Breaks when the player               | Site                                                   |
|-------------|--------------------------------------|--------------------------------------------------------|
| `Potions`   | drinks a potion                      | `break_use_conducts`, `models/src/items/mod.rs`            |
| `Scrolls`   | reads a scroll                       | `break_use_conducts`                                   |
| `Wands`     | zaps a wand, or throws one           | `break_use_conducts`; `resolve_wand_throw`, `throwing.rs` |
| `Runes`     | invokes a charged rune               | `break_use_conducts` (an inert rune does not count)    |
| `Spells`    | pays for a spell                     | `pay_for_spell`, `items/spells.rs`                     |
| `Coins`     | takes a coin                         | `spend_pickup`, `items/pickups.rs`                     |
| `Ranged`    | throws or fires anything             | `resolve_throw`, `items/throwing.rs`                   |
| `Attacking` | lands a melee swing, lunge or reach  | `land_swing`, `combat/`                              |
| `Charming`  | charms a creature                    | `charm`, `companion.rs`                                |
| `Companion` | recruits a boon companion            | `recruit`, `companion.rs`                              |
| `Damage`    | loses HP                             | `took_damage`, `helpers.rs`; the angel's test of faith (`angel_event`, `spirits.rs`) |
| `Gear`      | puts gear on                         | `toggle_equipped`, `equipment.rs`                      |
| `Armor`     | has armor take a killing blow        | `armor_takes_last_hit`, `helpers.rs`                   |
| `Lurk`      | is not the lurk                      | not tracked; read off the `Lurk` marker                |

`Gear` is deliberately not in `equip_silently`, so the starting kit does not break it, and taking gear off does not either. A charm scroll or wand breaks `Charming` and `Scrolls` or `Wands` together; a creature that becomes the Helper breaks `Companion`.

### Known gaps

`Damage` is "HP never dropped", and three paths lower the player's HP without going through `took_damage`: system shock from a polymorph (`system_shock`, `wands.rs`), a spirit's max-HP price (`pay`, `spirits.rs`), and the vampire's max-HP drain (`vampiric_drain`, `abilities.rs`). A run can keep `Damage` through them. The drain only matters when its hit did no damage. They are left unhooked on purpose.


The tally
---------

    pub enum Conduct { Potions, Scrolls, /* ... */ Lurk }
    pub struct Conducts { pub broken: u16 }

    pub fn broke(world: &mut World, conduct: Conduct)
    pub fn player_broke(world: &mut World, who: Entity, conduct: Conduct)
    pub fn kept_mask(world: &mut World) -> u16
    pub fn bonus_percent(mask: u16) -> i64

`Conducts` is a resource, one bit per `Conduct` in `Conduct::ALL` order; a set bit is a broken conduct. `broke` does nothing in a world with no `Conducts`, so a bare test world needs no setup. `player_broke` is `broke` behind a `Player` check.

`kept_mask` is what a run has kept: every conduct but the broken ones, with `Lurk` kept only when the player carries the `Lurk` marker. It is the one mask the payout, the leaderboard, the dump and the win screen all read.

The order of `Conduct::ALL` is a file format, because the leaderboard stores the mask by bit index. A new conduct goes on the end. Nothing is ever reordered.


Paying out
----------

    pub fn pay_conducts(world: &mut World) -> u16

    models/src/meta/score.rs

Settles the turn's kills, adds `bonus_percent(kept_mask)` of the score on top, and returns the mask it paid for. It is called once, by `run_victory_screens` in `engine/src/main.rs`, before the leaderboard is written, so the board, the dump and the win panel all see the bonused score. Calling it twice pays twice.


Where it is stored and shown
----------------------------

| Where           | What                                                                                       |
|-----------------|--------------------------------------------------------------------------------------------|
| Save            | `SaveGame::conducts`, the broken mask. Added with a `SAVE_VERSION` bump.                   |
| Leaderboard     | `Entry::conducts`, the kept mask (zero for a loss). `LEADERBOARD_VERSION` 2.               |
| Win screen      | One `conduct::MARK` per kept conduct, from `conduct::marks`, under the score.              |
| Run dump        | A `-- Conducts (+N%) --` section on a win, one name per kept conduct.                      |
| `-scores`       | The marks, uncoloured, after each line.                                                    |

A version 1 leaderboard is still read, with every mask zero, and becomes version 2 on the next write. Any other version is refused and left alone, as before.

`marks(mask, stripes)` colours the marks by cycling `stripes` by the conduct's place in `Conduct::ALL`, so a conduct keeps its colour whichever others are broken. The win screen passes the run's pride flag (`pride::stripes`). `Lurk` is always `LURK_COLOR`, the colour of the lurk's own `@`; a test fails if the two drift apart. The names come from `strings::conduct_name`, indexed by bit.


See also
--------

  ../../models/src/meta/constants/ the score constants the bonus sits on
  cli-and-env.md                -scores, the run dump and the data directory
  rendering.md                  the win panel the marks are drawn on
  ../explanation/ecs-in-nihilurk.md   resources, and why the tally is one
