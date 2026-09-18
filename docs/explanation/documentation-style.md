How nihilurk's documentation is written
===================================

    Audience       Anyone adding or editing a page under `docs/`, or
                   wondering why the page they are reading looks the way
                   it does.
    Prerequisites  None.
    This is        Understanding and a house rule, not a tutorial. It is
                   *inferred from the pages themselves* — every
                   convention below is one the existing docs already
                   keep, written down so the next page keeps it too.

The rules that already existed are in `../README.md`: four kinds of document kept strictly apart, and "documentation ships with the change that makes it true". This page is the layer under that — the shape of a page, and the voice inside it.

Nothing here is aesthetic for its own sake. Every convention exists because these pages are read in a terminal, next to the code, by someone who has a diff open.


The shape of a page
-------------------

### The header block

Every page but `docs/README.md` opens the same way: a setext title, then a four-space-indented block of aligned keys.

    Reference: content tables
    =========================

        Audience       Anyone editing content. Look things up here; do
                       not read it end to end.
        Prerequisites  None.
        Status         Describes the tables as they are in the source.
                       If this page and the source disagree, the source
                       is right and this page is a bug.

`Audience` and `Prerequisites` are on every page. The third key depends on which of the four kinds you are writing, and it is not decoration — it is the page telling the reader what promise it is making:

| Directory      | Third key | What it promises                        |
|----------------|-----------|-----------------------------------------|
| `reference/`   | `Status`  | It tracks the source, and the source wins. |
| `explanation/` | `This is` | It is reasoning, not instructions.      |
| `how-to/`      | *(none)*  | Nothing beyond the task in the title.   |
| `tutorial/`    | *(none)*  | Nothing beyond the lesson.              |

Values wrap under the key, aligned at column 20. A how-to that needs to point somewhere else does it in the opening paragraph, not by growing a third key.

**An ADR is the one exception**, and it keeps the shape ADRs have everywhere: `Status` (accepted / superseded), `Audience`, `Supersedes`, `Related`, and no `Prerequisites`. It is a record with a lifecycle rather than a page about the code, and the header says which.

The `Status` formula is load-bearing and should be copied verbatim: *"If this page and the source disagree, the source is right and this page is a bug."* It is the sentence that makes a stale reference page a defect rather than an opinion.

### Headings

Setext for the top two levels, `###` for the third, and nothing below that:

    Page title
    ==========

    A section
    ---------

    ### A subsection

There is not a single `#` or `##` in `docs/`. Four levels of heading in a page this size means the page is two pages.

### Line width: there isn't one

**A paragraph is one line.** Do not hard-wrap prose. These pages are read rendered as often as they are `cat`ed, a rendered paragraph reflows to whatever the reader's window is, and a hand-wrapped one only means the next person to edit a sentence has to rewrap the four lines after it. Nothing checks the width, and nothing should.

What *is* still line-sensitive, and is therefore left exactly alone: fenced blocks, four-space code blocks, tables, the header block at the top of a page, and the `See also` block. A bullet is one line too, continuations folded in.

### Diagrams

A page that describes a **flow** — an order things happen in, a stack of contexts, a chain of steps — opens with a Mermaid diagram before the prose that explains it. Horizontal (`flowchart LR`), so it reads the way the text under it does, and themed so every diagram in the tree looks like the same game.

Copy the theme block verbatim from any existing diagram; it is the terminal's own palette, which is the only palette nihilurk has:

    ```mermaid
    %%{init: {'theme':'base','themeVariables':{
      'primaryColor':'#20242b','primaryTextColor':'#d7dae0',
      'primaryBorderColor':'#5c6370','lineColor':'#8a8f98',
      'fontFamily':'ui-monospace, SFMono-Regular, Menlo, monospace',
      'fontSize':'13px'}}}%%
    flowchart LR
      A[input] --> B[turn] --> C[frame]
      classDef hero fill:#3a3418,stroke:#d7ba4a,color:#e8dfa8
      classDef peril fill:#3a1f1f,stroke:#c05050,color:#f0c8c8
      classDef magic fill:#2f2038,stroke:#a86fc0,color:#e6cdf0
      classDef cold  fill:#17323a,stroke:#4aa3c0,color:#bfe4f0
    ```

Four accent classes, and they mean what they mean in the game: `hero` yellow for the player and their own actions, `peril` red for damage and death, `magic` magenta for items and effects, `cold` cyan for the machinery around them (stairs, the schedule, the renderer). Leave a node unclassed for anything that is none of those.

A diagram is a summary, never the specification — the prose under it still has to say everything. If a diagram needs more than about a dozen nodes, the page is describing two flows.

### Code

Indent four spaces. Indentation survives `cat`, `less` and a paste into a commit message; a fence does not, and a fence in a terminal is three backticks of noise.

The one exception the corpus keeps is in `reference/`, which fences a bare **declaration** — a signature, a struct shape — as a heading for the paragraph that explains it, the way this page's own example a moment ago did. Those are labels rather than code: they are allowed to elide a parameter list down to `world, stdout, screen`. Examples, shell commands, table sketches and ASCII diagrams are indented, everywhere, always.

Shell examples carry a comment saying what the command answers:

    cargo test --test content                 # the tables specifically
    cargo run -p engine -- -content           # what the game knows

### Tables

GFM pipe tables, column-padded so the source is readable unrendered. A table earns its place when the rows have the same shape — fields of a struct, keys of a menu, variants of an enum. Three columns is the usual number; past five it is a list wearing a costume.

### The `See also` block

Last section on every page — except a tutorial, which points *forward* rather than sideways and ends with `Where to go next` instead. Same shape, different promise. Two-space indent, target then a lower-case fragment saying what is over there, aligned:

    See also
    --------

      content-tables.md             the tables these functions read
      cli-and-env.md                NIHILURK_SPAWN and the -content flag
      ../how-to/spawn-a-thing.md    recipes for the functions above

Paths are relative to the page. Never a bare filename with no gloss: the gloss is the whole point, because it is what tells a reader whether to follow the link.


The voice
---------

### Say what a thing is *for*, not what it is

The house habit, everywhere, is to answer "why would I care" in the same breath as "what is it":

> `Projectile` means three things at once: the throw ignores the target's
> armour die, the missile is spent on what it hits, and nothing can catch
> it.

not "a marker component indicating projectile status".

A field's *type* is already in the source. What the page adds is the consequence.

### Second person, present tense, active

"You change the value in `constants.rs` and nowhere else." The reader is a person with a diff open, and the docs talk to them.

The exception is `reference/`, which describes the program rather than addressing the reader: "`pack_rows` returns backpack indices."

### Prefer the concrete noun to the abstract one

nihilurk's docs say "a dragon", "a red coin at full health", "a corridor full of kobolds" where a lesser page would say "an entity", "an unusable pickup", "multiple targets". The specific case is what makes a rule memorable, and the rule is usually general anyway.

### Name the thing that would go wrong

Most paragraphs that explain a decision end by naming the failure the decision prevents:

> Without this, drinking a potion of blindness would be a way to hide.

> A test that says a wand rolls `3..=9` is not testing the game; it is
> testing `constants.rs`, which needs no help.

If you cannot name what goes wrong without the rule, you may not have a rule.

### Bold the rule, italicise one word

**Bold** opens a paragraph that states a rule (`**A full pack is no obstacle.**`). *Italics* fall on the single word carrying the contrast (*where*, not *what*). Neither is used for emphasis in general; a page with bold in every paragraph has none.

### British spelling in prose, the code's spelling in code

"Colour", "armour", "recognise" in sentences; `Color`, `armor_bonus`, `recognised` inside backticks, because that is what you would grep for. The seam is the backtick.

### Dashes

Both `--` and `—` are in use, and both are fine. Do not mix them *within a page*: pick whichever the page already has and stay with it.


What a page must not do
-----------------------

  * **Restate a tuning number.** Name the constant — `AMMO_BUNDLE_MIN..=AMMO_BUNDLE_MAX`, not "3 to 12". A number copied into prose is a number that goes stale the first time somebody rebalances, and nihilurk has had every one of them go wrong at least once. The same rule holds for doc comments in the source, and for tests (`code-calisthenics.md`, "a test never asserts a constant").
  * **Paste a list the program can print.** `cargo run -p engine -- -content` reads the tables, so it can never be wrong. Point at it.
  * **Be two kinds of document at once.** A how-to that starts explaining itself is a how-to and an explanation; split it and link. That rule is in `../README.md` and this page is the result of following it — the ECS pages are a how-to and an explanation, not one page.
  * **Document a deliberate secret.** The `T` key and the `-pride` flags stay out of `MANUAL.md` and out of `../reference/cli-and-env.md` on purpose. Where a page has to mention one — because an engine developer will meet it in the source — it says *keep it out of the manual* in as many words.
  * **Go stale quietly.** If a claim can be a test, make it a test and cite the test. `models/tests/content.rs` exists to hold up the claims `how-to/` makes about the tables.
  * **Qualify a path it cannot keep.** `` `models/src/map/levels.rs` `` is a claim about where something lives, and `docs_style.sh` checks it. A bare `` `levels.rs` `` is prose and is not checked — so name the directory when the reader needs it, and drop it once context has established where you are.


Checking a page
---------------

Every rule above is checked by one script, so none of this has to be remembered:

    ./docs_style.sh                   every page under docs/
    ./docs_style.sh docs/how-to       one directory, or one file
    ./docs_style.sh --strict          legacy overruns fail too

It reports and never rewrites, because where a check fires the fix is a judgement call about where a sentence should break. Nine checks, one per section of this page; its own header comment says which is which, and why a markdown formatter is the wrong tool for this particular corpus.

Exit status is 0 when everything passes, so it drops into a pre-commit hook or a CI step as `./docs_style.sh || exit 1`.

A handful of over-width lines predate this page being written. They are inside a budget the script holds, so a *new* one fails immediately; the budget comes down as pages get touched for other reasons.


See also
--------

  ../README.md                 the four kinds of document, and the index
  code-calisthenics.md         the same discipline, applied to the code
  data-driven-content.md       why the docs describe tables, not classes
  ../../MANUAL.md              the player-facing document, a different voice
