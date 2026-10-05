-- lua .githooks/docs_nudge_test.lua
--
-- Runs docs_nudge.lua inside a throwaway git repo with different changes
-- staged. The nudge names the pages a change makes false, only while those
-- pages are not staged too, and never fails a commit.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local root = io.popen("cd '" .. here .. "' && pwd"):read("l")

local function sh(cmd)
  local p = assert(io.popen(cmd .. " 2>&1", "r"))
  local out = p:read("a")
  local _, _, code = p:close()
  return out, code
end

local dir = os.tmpname()
os.remove(dir)
assert(os.execute("mkdir -p '" .. dir .. "/.githooks'"))
assert(os.execute("cp '" .. root .. "/docs_nudge.lua' '" .. root .. "/docs_nudge_rules.lua' '" .. dir .. "/.githooks/'"))
sh("cd '" .. dir .. "' && git init -q && git config user.email t@t && git config user.name t")

local function write(name, text)
  assert(os.execute("mkdir -p \"$(dirname '" .. dir .. "/" .. name .. "')\""))
  local f = assert(io.open(dir .. "/" .. name, "w"))
  f:write(text)
  f:close()
end

local function git(args)
  return sh("cd '" .. dir .. "' && git " .. args)
end

local function nudge()
  return sh("cd '" .. dir .. "' && lua .githooks/docs_nudge.lua")
end

local BASE = {
  ["models/src/components.rs"] = "pub enum RingEffect {\n    Protection,\n    Strength,\n}\n\npub enum SpellEffect {\n    Sting,\n}\n",
  ["models/src/constants.rs"] = "pub mod player {\n    pub const START_HP: i32 = 8;\n}\n",
  ["engine/src/main.rs"] = "fn main() {\n    match arg {\n        \"-ns\" => no_save = true,\n    }\n}\n",
  ["models/src/catalog.rs"] = "// Weapons.\npub const WEAPONS: &[u8] = &[];\n",
  ["models/src/agents.rs"] = "pub const STRIKE: Rule = Rule {};\n",
  ["models/src/spawn.rs"] = "pub const DROPS: &[u8] = &[\n    category!(\"scroll\", 300, 1, SCROLLS),\n];\n",
  ["docs/reference/components.md"] = "x\n",
}
for name, text in pairs(BASE) do write(name, text) end
git("add -A")
git("commit -qm base")

local function reset()
  git("reset -q --hard HEAD")
end

local function change(name, from, to)
  local f = assert(io.open(dir .. "/" .. name))
  local s = f:read("a")
  f:close()
  local i, j = s:find(from, 1, true)
  assert(i, "anchor missing: " .. from)
  write(name, s:sub(1, i - 1) .. to .. s:sub(j + 1))
  git("add -- '" .. name .. "'")
end

-- 1. A new ring names the page that pastes the enum, and does not fail.
change("models/src/components.rs", "    Strength,\n", "    Strength,\n    Polymorph,\n")
local out, code = nudge()
assert(code == 0, "the nudge never fails a commit, got " .. tostring(code))
assert(out:find("docs/reference/components.md", 1, true), "ring should name components.md, got: " .. out)
assert(out:find("update-the-docs.md", 1, true), "should point at the lookup page, got: " .. out)

-- 2. Staging that page too silences it.
change("docs/reference/components.md", "x\n", "x\ny\n")
out, code = nudge()
assert(code == 0 and out == "", "page staged should be silent, got: " .. out)
reset()

-- 3. A new constants module names the modules table.
change("models/src/constants.rs", "pub mod player {", "pub mod spells {\n}\n\npub mod player {")
out = nudge()
assert(out:find("docs/reference/constants.md", 1, true), "constants module, got: " .. out)
reset()

-- 4. A new flag names the CLI page and the man page.
change("engine/src/main.rs", '        "-ns" => no_save = true,\n', '        "-ns" => no_save = true,\n        "-zz" => zz = true,\n')
out = nudge()
assert(out:find("docs/reference/cli-and-env.md", 1, true), "flag, got: " .. out)
assert(out:find("doc/nihilurk.6", 1, true), "flag should name the man page, got: " .. out)
reset()

-- 5. A comment edit, and an enum with no page, say nothing.
change("models/src/catalog.rs", "// Weapons.", "// The weapons.")
out, code = nudge()
assert(code == 0 and out == "", "a comment edit should be silent, got: " .. out)
reset()
change("models/src/components.rs", "    Sting,\n", "    Sting,\n    IceBolt,\n")
out, code = nudge()
assert(code == 0 and out == "", "SpellEffect has no page to touch, got: " .. out)
reset()

-- 6. An empty commit is silent.
out, code = nudge()
assert(code == 0 and out == "", "nothing staged should be silent, got: " .. out)

-- 7. A new component, rule, rule set or item category names its page.
change("models/src/components.rs", "pub enum SpellEffect {", "pub struct Fresh;\n\npub enum SpellEffect {")
out = nudge()
assert(out:find("docs/reference/components.md", 1, true), "a new struct, got: " .. out)
reset()
change("models/src/agents.rs", "pub const STRIKE: Rule = Rule {};\n", "pub const STRIKE: Rule = Rule {};\npub static COWARD: RuleSet = RuleSet {};\n")
out = nudge()
assert(out:find("docs/reference/agents.md", 1, true), "a new rule set, got: " .. out)
reset()
change("models/src/agents.rs", "pub const STRIKE: Rule = Rule {};\n", "pub const STRIKE: Rule = Rule {};\npub const BOLT: Rule = Rule {};\n")
out = nudge()
assert(out:find("docs/reference/agents.md", 1, true), "a new rule, got: " .. out)
reset()
change("models/src/spawn.rs", '    category!("scroll", 300, 1, SCROLLS),\n', '    category!("scroll", 300, 1, SCROLLS),\n    category!("treat", 40, 1, TREATS),\n')
out = nudge()
assert(out:find("docs/reference/content-tables.md", 1, true), "a new category, got: " .. out)
assert(out:find("docs/how-to/add-an-item.md", 1, true), "a new category should name add-an-item.md, got: " .. out)
reset()

os.execute("rm -rf '" .. dir .. "'")

-- 8. update-the-docs.md and the nudge agree. The page is the spec; the rules
-- are what a diff can recognise of it. A row with pages to touch has a rule
-- naming exactly those pages, or a stated reason no diff can see it.
local repo = root .. "/.."
local ok, spec = pcall(dofile, root .. "/docs_nudge_rules.lua")
assert(ok, "docs_nudge_rules.lua should load: " .. tostring(spec))

local ROOT_FILES = { ["MANUAL.md"] = true, ["gdd.md"] = true }
local function page_of(token)
  if token == "doc/nihilurk.6" then return token end
  if not token:match("%.md$") then return nil end
  if ROOT_FILES[token] then return token end
  if token:match("^%.%./%.%./") then return token:sub(7) end
  if token:match("^%.%./") then return "docs/" .. token:sub(4) end
  return "docs/how-to/" .. token
end

local function table_of(path)
  local rows = {}
  local f = assert(io.open(path))
  for l in f:lines() do
    local label, touch = l:match("^| ([^|]-) +| ([^|]-) +|")
    if label and not label:match("^%-") and label ~= "You added or changed" then
      local pages = {}
      for token in touch:gmatch("`([^`]+)`") do
        local page = page_of(token)
        if page then pages[page] = true end
      end
      rows[label] = pages
    end
  end
  f:close()
  return rows
end

local function sorted(set)
  local out = {}
  for k in pairs(set) do out[#out + 1] = k end
  table.sort(out)
  return table.concat(out, ", ")
end

local function problems(rules, unwatched, rows)
  local found, covered = {}, {}
  for _, rule in ipairs(rules) do
    local pages = rows[rule.row]
    if not pages then
      found[#found + 1] = "rule names a row the page lacks: " .. rule.row
    else
      if covered[rule.row] then found[#found + 1] = "two rules for one row: " .. rule.row end
      covered[rule.row] = true
      local mine = {}
      for _, p in ipairs(rule.pages) do mine[p] = true end
      if sorted(mine) ~= sorted(pages) then
        found[#found + 1] = rule.row .. ": rule pages [" .. sorted(mine) .. "] but the page says [" .. sorted(pages) .. "]"
      end
    end
  end
  for label, pages in pairs(rows) do
    if next(pages) and not covered[label] and not unwatched[label] then
      found[#found + 1] = "row with pages and no rule or reason: " .. label
    end
  end
  for label in pairs(unwatched) do
    if not rows[label] then found[#found + 1] = "stale reason for a row that is gone: " .. label end
    if covered[label] then found[#found + 1] = "row has both a rule and a reason: " .. label end
  end
  return found
end

local rows = table_of(repo .. "/docs/how-to/update-the-docs.md")
assert(next(rows), "found no rows in update-the-docs.md")
local found = problems(spec.rules, spec.unwatched, rows)
assert(#found == 0, "update-the-docs.md and docs_nudge_rules.lua disagree:\n  " .. table.concat(found, "\n  "))
for _, rule in ipairs(spec.rules) do
  for _, page in ipairs(rule.pages) do
    assert(io.open(repo .. "/" .. page), "a rule names a page that is not there: " .. page)
  end
end

-- The check has to bite: doctor each side and it must object.
local function copy(rules)
  local out = {}
  for i, r in ipairs(rules) do
    local c = {}
    for k, v in pairs(r) do c[k] = v end
    out[i] = c
  end
  return out
end
local trimmed = copy(spec.rules)
trimmed[1].pages = { "docs/reference/agents.md" }
assert(#problems(trimmed, spec.unwatched, rows) > 0, "a rule with the wrong pages should be caught")
local missing = copy(spec.rules)
table.remove(missing, 1)
assert(#problems(missing, spec.unwatched, rows) > 0, "a row with no rule and no reason should be caught")
local extra = {}
for label, pages in pairs(rows) do extra[label] = pages end
extra["A brand new row"] = { ["docs/reference/components.md"] = true }
assert(#problems(spec.rules, spec.unwatched, extra) > 0, "a new row nobody covered should be caught")
local stale = {}
for label, why in pairs(spec.unwatched) do stale[label] = why end
stale["A row that is gone"] = "x"
assert(#problems(spec.rules, stale, rows) > 0, "a stale reason should be caught")

print("docs_nudge_test: ok")
