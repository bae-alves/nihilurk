-- What a staged diff can recognise of docs/how-to/update-the-docs.md. Read by
-- docs_nudge.lua, and checked against that page by docs_nudge_test.lua: a row
-- with pages to touch has a rule here naming exactly those pages, or a reason
-- in `unwatched`. Add a row to the page and the test sends you here.
--
-- A rule fires on an added or removed line of `file`. `under` narrows it to
-- hunks whose top-level item matches (git puts that item in the hunk header,
-- so `pub enum RingEffect {` names the enum a new variant joined). `line` is
-- a Lua pattern, a list of them, or a function of the diff line.

local COMPONENTS = "docs/reference/components.md"
local CONSTANTS = "docs/reference/constants.md"
local TABLES = "docs/reference/content-tables.md"
local FEEL = "docs/explanation/the-feel-layer.md"
local CLI = "docs/reference/cli-and-env.md"
local AGENTS = "docs/reference/agents.md"
local MAN = "doc/nihilurk.6"
local MANUAL = "MANUAL.md"

local function variant(line) return line:match("^[+-]    %u%w*,") ~= nil end

local function enum_variant(row, enum, pages)
  return { row = row, file = "models/src/components.rs", under = "^pub enum " .. enum, line = variant, pages = pages }
end

return {
  rules = {
    enum_variant("A ring", "RingEffect", { COMPONENTS }),
    enum_variant("A potion", "PotionEffect", { COMPONENTS }),
    enum_variant("A scroll", "ScrollEffect", { COMPONENTS, FEEL }),
    enum_variant("A wand", "WandEffect", { COMPONENTS, TABLES, FEEL }),
    enum_variant("A trap", "TrapEffect", { COMPONENTS }),
    { row = "An effect", file = "models/src/effects.rs", line = '^%+%s+"[%l_]+" => ', pages = { TABLES } },
    { row = "A playable body", file = "models/src/body.rs", under = "^pub enum Body", line = variant,
      pages = { CLI, COMPONENTS, CONSTANTS, MANUAL, MAN } },
    { row = "A rule", file = "models/src/agents.rs", line = "^%+pub const [%u_]+: Rule[ =]", pages = { AGENTS } },
    { row = "A rule set", file = "models/src/agents.rs", line = "^%+pub static [%u_]+: RuleSet", pages = { AGENTS } },
    { row = "A whole item category", file = "models/src/spawn.rs", line = "^[+-]%s+category!%(",
      pages = { TABLES, "docs/how-to/add-an-item.md", "docs/how-to/tune-rarity-and-depth.md" } },
    { row = "A `constants::` module", file = "models/src/constants.rs", line = "^%+pub mod ", pages = { CONSTANTS } },
    { row = "A component, resource or event", file = "models/src/components.rs",
      line = { "^%+pub struct ", "^%+pub enum " }, pages = { COMPONENTS } },
    { row = "A key", file = "engine/src/update.rs", line = "^%+%s+KeyCode::Char%('.'%) =>",
      pages = { MANUAL, MAN, "docs/reference/input-and-turn-loop.md" } },
    { row = "A flag or an env var", file = "engine/src/main.rs", line = '^%+%s+"%-%l+"', pages = { CLI, MAN } },
  },

  -- Rows with pages to touch that no changed line gives away.
  unwatched = {
    ["A kind of entity that is not an item, monster or trap"] = "no line marks a new kind of entity",
    ["A system in the turn schedule"] = "pre-commit check 2 already refuses a changed `.after()` edge until a page is staged",
    ["A new page"] = "an added file, not a changed line",
  },
}
