# Plan: say what is different, and keep a borrow-list

## Context

bae wants `gdd.md` and `MANUAL.md` to say how nihilurk differs from the TUI roguelikes a player has likely met, plus a local note, "concepts from other games that could be in nihilurk", written against the creed **minimum input -> maximum output**.

**The comparison set is six games:** Rogue, NetHack, Dungeon Crawl Stone Soup, Angband, Umoria, and Brogue (`github.com/tmewett/BrogueCE`, added by bae). Nothing else is compared.

Status:
- **Done** — player docs of Rogue, NetHack, Crawl, Angband, Umoria (via `gh api`).
- **Done, supplementary** — a `head -50` pass over nihilurk's 168 non-doc text files, only to know what nihilurk already has.
- **Not done** — the `head` pass over each classic's code, the full reads bae named, and everything about Brogue. Step 0.

bae edits `gdd.md` and `MANUAL.md` while I work (bae added a Spirits paragraph to both: `gdd.md:58`, `MANUAL.md:130`). Re-read before every edit, edit by anchored string, never rewrite those paragraphs.

## Rules from bae (they override anything older)

1. **No "none of the five does X" in any doc.** Comparisons are fine ("you may expect rest here; you get stairs"), absolute claims are not. This includes the current `gdd.md` sentence "The trick shot is the thing no other roguelike does": cut that clause.
2. **Automation is a focus, not a differential.** `o`, `Tab`, travel and `>`/`<` pathing follow Crawl, which is built the same way. Say "focus" and credit Crawl.
3. **The game has classes: they are called bodies** (nihil, lurk, any monster via `-am`). Never write "classless". Keep "no experience levels".
4. **Stairs are the only non-magical recovery.** There is no resting, searching or passing a turn, and no natural regeneration. The ring of regeneration does not heal HP: each turn it has a chance to clear one condition, else restore one drained power (`items/rings.rs::regenerate`).
5. **Everything but gear is pre-identified.** Potions, scrolls, wands, runes, rings, coins, treats. Gear hides its plus and its curse until worn or identified.
6. **Not differentials, so out of the docs:** combat math versus D&D, seed-fixed layouts, the bones ghost.
7. **No hunger is true** (Crawl has none either). **The patience timer comes from Crypt of the NecroDancer**; credit it.
8. **Decks of cards are Crawl's**; throwing a deck so it plays as a poker hand is nihilurk's own. Credit Crawl.
9. **Particles claim, exactly:** portable particle arithmetic in a reusable effects crate (`particle-core`: `no_std`, no dependencies, built for RISC-V and ESP32 by `compat/`). It goes in "Technical Description". The feel layer itself is meant to be experienced, so it stays out of the differences section; say only that the gorefest is opt-out by argument (`-nb`, `-nshake`).
10. **Moddability is a flex, said lightly:** a human-friendly moddable codebase *in Rust*. Point at `docs/`, which follows Diátaxis (tutorial, how-to, reference, explanation). No comparison to other games' modding.
11. **Env vars are not a wizard mode** but the sentence must mention the contrast: `NIHILURK_SPAWN`, `NIHILURK_LEVEL`, `NIHILURK_MAGICMAP` (documented in `docs/reference/cli-and-env.md`).
12. **No start menus: "like Rogue".** Name and body are arguments.
13. **Move the trick-shots block into `### What is different`.**
14. **Appendix N: bare list. bae writes the whys.** Do not invent reasons.
15. **Do not mention coin colours.** Describe coin effects only.
16. **Fix the stale "unidentified items"** in `MANUAL.md:4`, `README.md:10`, `site/llms.txt:3`.
17. **No mouse support exists.** My earlier note was wrong: `engine/src/view.rs` only filters stray mouse reports while an animation plays, and `EnableMouseCapture` appears nowhere. "Mouse click to travel" stays a candidate idea, labelled "not supported today".
18. Never mention `-pride` or the `T` key (deliberate secrets, `docs/explanation/documentation-style.md`).

## Steps

### 0. Research still owed (first; it can change a claim)

Scratch dir outside the repo, one new empty dir per game under `/tmp/classics-heads/<game>/`, scripts kept elsewhere, bash or Lua only, never committed, nothing run from inside a download dir. Trees listed with `gh api repos/<r>/git/trees/<ref>?recursive=1`; file bodies fetched with `curl` from `raw.githubusercontent.com` in parallel (`xargs -P`), not the API.

**a. Head pass over each classic's non-doc text files.**

| Repo | Text files (approx.) | Lines each |
|---|---|---|
| Davidslv/rogue (`modern-rogue`) | 36 | 50 |
| NetHack/NetHack | 870 | 50 |
| crawl/crawl | 1,550 (of 11,489 blobs) | **10** (bae: Crawl would splurge tokens) |
| angband/angband | 650 | 50 |
| dungeons-of-moria/umoria | 85 | 50 |
| tmewett/BrogueCE | 55 source of 100 blobs | 50 |

Skip images, sounds, fonts, saves and vendored third-party code (Crawl `contrib/`, bundled Lua/SQLite/PCRE/libpng). One digest per game: path, then the lines, with licence blocks and blank runs squeezed so tokens go to what carries meaning. Digests are read in chunks, never dumped raw. The squeeze and the Crawl cut are stated in the note.

**b. Full reads of the docs I had only grepped or skipped, as bae named:**
- Crawl: `options_guide.txt` (161 KB), `changelog.txt` (524 KB, old entries included), and the rest of `crawl-ref/docs/` (`arena`, `fight_simulator`, `macros_guide`, `ssh_guide`, `tiles_help`, `develop/*`).
- NetHack: every `doc/fixes*.txt`, `dat/tribute`, `dat/history`, `dat/help`, `dat/opthelp`, `doc/config.nh`, `README`.
- Umoria: `historical/errors.md`, `history.md`, the old `historical/CHANGELOG`, `dragons.md`, `README.md`.
- Angband: `thanks.rst`, `copying.rst`, `docs/hacking/*.rst`, `customize.rst`, `version.rst`, `changes.txt`, `src/doc/*`.
- Rogue: `BUILD_ISSUES.md`, `MODERNIZATION_LOG.md`, rest of `README.md`.
- Brogue: `README.md`, `BUILD.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `changes/*.md`, `bin/keymap.txt`.

Read in full (blank lines squeezed only). Rough cost 1.5-2M tokens, inside budget.

**c. Use it.** Test every comparative claim against the digests; mine them for borrow-list ideas (Brogue's are not pre-judged here); rewrite any claim they contradict before it reaches a doc. Remove the scratch dirs at the end after looking at them.

### 1. `gdd.md` (re-read first)

- Cut the "no other roguelike does" clause (rule 1). Gameplay paragraph: automation becomes "a focus", credit Crawl (rule 2).
- Replace `### What is unique: trick shots` with **`### What is different`**, trick-shots block moved in whole (rule 13). Opens with one line naming the six games. Then bold run-in labels, in the GDD's voice (first person, American spelling, numbers allowed):
  - **Trick shots.** The existing block, unchanged apart from rule 1.
  - **Also different:** wand grenades (a thrown wand spends every charge and bursts); coins as pickups; spirits (point at the paragraph at line 58); boon companions (throw the right treat; one at a time; follows you down); a thrown deck plays as a poker hand (decks are Crawl's); play as any monster (`-am`); bodies are the classes; stairs are the only non-magical recovery, and the ring of regeneration clears conditions; everything but gear is pre-identified; no hunger, rest or search; like Rogue, the game starts at once with no menus, name and body as arguments; env vars `NIHILURK_SPAWN`, `NIHILURK_LEVEL`, `NIHILURK_MAGICMAP` where the classics ship a wizard mode; the gorefest is opt-out by argument.
  - **Borrowed, said so:** Crawl for the automation focus and for decks; NetHack for shift-run; Crypt of the NecroDancer for the patience timer; Rogue for sight, the room generator and permadeath.
  - **A flex, said lightly:** a human-friendly moddable codebase in Rust; `docs/` is Diátaxis.
- "Technical Description": add the particle-arithmetic sentence (rule 9).

### 1a. `gdd.md` Appendix N (last section, after "Other ideas/Expansion backlog")

Heading `APPENDIX N - Games that Inspired This`. Bare bullets, one title per line (so "Dicing Knight." keeps its full stop). **Every game studied is listed**, plus bae's list.

- Studied roguelikes: Rogue; NetHack; Dungeon Crawl Stone Soup; Angband; Umoria; Brogue.
- Wildfrost; Crypt of the NecroDancer; Slay the Spire; Balatro; Dwarf Fortress; XCOM; the Nier series; Dicing Knight.; everything From Software; Darkest Dungeon; Nuclear Throne; The Binding of Isaac; the Mystery Dungeon series; Super Auto Pets; One Way Heroics; Spelunky; WazHack; Caves of Qud.
- Subsection **Analog Games**: Ironsworn; Tunnels and Trolls (1975 edition); Knave 2e; Heroes of Cerulea; OSRIC and the retroclone ecosystem, which is what made bae try to make videogame retroclones.

No whys: bae fills them.

### 2. `MANUAL.md` (re-read first; player voice, plain words, no passive)

- Line 4: fix "unidentified items" (rule 16).
- After the trick-shot intro paragraph: `If you have played other roguelikes`, a two-column table, `You may expect` / `Here`: resting and searching; a food clock; levels; unknown potions and scrolls; shops; start-up menus (like Rogue, none); pets; wands; gold. Only comparisons, no absolutes.
- Short sections the manual lacks (grep confirmed `Helper`, treats, wand grenades, `-am`, env vars are absent): Helpers; thrown wands; coins (effects only, rule 15); `-am <monster>` and the env vars, two lines under "Starting the game". The existing line about turning off blood and shake stays; no feel-layer prose.
- Stairs as the only recovery gets one plain sentence in "Moving and fighting".

### 3. Stale wording

`README.md:10` and `site/llms.txt:3`, same fix as `MANUAL.md:4`: say gear of unknown quality, not unidentified items. Leave `docs/` alone: its uses are about gear and are correct.

### 4. `.claude/concepts-from-other-games.md` (new, local, for future Claude sessions; CLAUDE.md prose rules)

- Top: the creed, and the test every idea must pass: keys per use, what comes back, fit with "cannot pass a turn".
- *Sources and gaps*: everything read in step 0, the squeeze, the Crawl `head -10`, the skipped vendored trees.
- *Already in nihilurk*: automation, bones, a reticle that opens on the nearest foe, the `A` pickup toggle, `-endless`, env-var content shortcuts.
- *Worth borrowing*, ranked by output per input; each with source doc or file, input cost, output, risk, and the nihilurk file it would touch. Seeds from my reading so far (all absent in nihilurk by grep), to be re-ranked after step 0 and joined by Brogue's:
  1. Level feeling on arrival (Angband `^f`, `birth_feelings`): 0 keys.
  2. Run dump on death (Crawl `dump_on_save`, Angband character dump, Umoria death record): 0 keys.
  3. `-daily` seed (Crawl "Seeded play": seeded runs scored separately).
  4. Conducts and achievements as score (NetHack): 0 keys; must not reward a boring safe line (Crawl "Crusade against no-brainers").
  5. Cursor jumps in `O` and look: `<` `>` stairs, `x` nearest unexplored (Angband 4.2.6, Crawl); `travel_cursor_step` has direction keys only.
  6. Repeat last action (NetHack `^A`, Crawl `p`/`f` previous target).
  7. Mouse click to travel and aim (Angband `mouse_movement`, NetHack `_`): **not supported today**; needs `EnableMouseCapture`.
  8. Darkness that deepens with depth (Angband guide): 0 keys.
  9. Named uniques with escorts and a fixed drop (Angband guide, Crawl "Uniques").
  10. First-run hints (Crawl tutorial and hints mode).
- *Rejected, with the reason*: hunger, rest and search, shops, XP, identify-by-use, start menus, persistent levels, wizard mode, prayer, Sokoban.

### 5. Keep the note untracked without `.gitignore`

Append `/.claude/concepts-from-other-games.md` to `.git/info/exclude` (bae's choice; the existing `.gitignore` already covers `*.local.md`, and this file deliberately is not one).

### 6. Journal

Dated entry in `.claude/journal.local.md` (git-ignored): automation is Crawl's focus; the six sources and where they were read; the concurrent-edit note; the mouse correction.

## Critical files

Edit: `gdd.md`, `MANUAL.md`, `README.md`, `site/llms.txt`, `.git/info/exclude`, `.claude/journal.local.md`. Create: `.claude/concepts-from-other-games.md`. Read for facts before writing: `models/src/spirits.rs` (event docs, for the cast list), `models/src/items/pickups.rs` and `COINS` in `models/src/catalog.rs` (effects only), `models/src/companion.rs`, `docs/reference/cli-and-env.md`, `docs/reference/content-tables.md` (reuse its Helpers and Spirits wording).

No code changes, so no TDD step.

## Verification

- `git status --short`: `gdd.md`, `MANUAL.md`, `README.md`, `site/llms.txt` modified; the concepts note absent. `git check-ignore -v .claude/concepts-from-other-games.md` names `.git/info/exclude`.
- `grep -n -i 'unidentified items'` over the three fixed files: zero hits.
- `grep -n -i -E 'no other roguelike|none of the (five|six)|classless|\-pride'` over `gdd.md`, `MANUAL.md`: zero hits. No `T` key mention.
- `lua site/build_test.lua` (the site builds from `MANUAL.md`).
- Appendix N lists all six studied games.
- Play check with the `play-nihilurk` skill: `NIHILURK_SPAWN` a spirit and a treat, bump and throw, confirm every new MANUAL sentence against the screen.
- `git diff -U0 gdd.md MANUAL.md`: only my hunks beside bae's.

## Risks and gaps (reported after the work)

- **Medium** — concurrent edits by bae; mitigated by re-read and anchored edits.
- **Medium** — 50 lines (10 for Crawl) show what a file is for, not all it does. The note says "the docs and file headers show no X", never "the code has no X".
- **Medium** — the digest squeeze and the Crawl `head -10` deviate from a literal `head -50`; the note says so.
- **Low** — the Brogue docs and code are unread today, so nothing about Brogue is claimed until step 0 is done.
- **Low** — "pre-identified: everything but gear" assumes coins and treats count as pre-identified; confirmed by the `play-nihilurk` check before the sentence ships.
