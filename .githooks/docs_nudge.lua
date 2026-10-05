-- Reminds the committer which docs a staged change makes false. Called by
-- .githooks/pre-commit, after the checks that can refuse. It never refuses:
-- it prints, then exits 0. The rules are in docs_nudge_rules.lua, and
-- docs_nudge_test.lua checks them against docs/how-to/update-the-docs.md.
--
-- A rule fires on a staged added or removed line, and falls silent once every
-- page it names is staged too.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local RULES = dofile(here .. "/docs_nudge_rules.lua").rules

local function lines_of(cmd)
  local p = assert(io.popen(cmd))
  local out = p:read("a")
  p:close()
  return out
end

local function matches(rule, raw)
  local line = rule.line
  if type(line) == "function" then return line(raw) end
  for _, pattern in ipairs(type(line) == "table" and line or { line }) do
    if raw:match(pattern) then return true end
  end
  return false
end

local staged = {}
for name in lines_of("git diff --cached --name-only --diff-filter=ACMR"):gmatch("[^\n]+") do
  staged[name] = true
end

local reminders, order = {}, {}
local function remind(rule)
  local missing = {}
  for _, page in ipairs(rule.pages) do
    if not staged[page] then missing[#missing + 1] = page end
  end
  if #missing > 0 and not reminders[rule.row] then
    reminders[rule.row] = missing
    order[#order + 1] = rule.row
  end
end

local file, item
for raw in lines_of("git diff --cached -U0 --no-color --diff-filter=ACMR"):gmatch("[^\n]*") do
  local path = raw:match("^%+%+%+ b/(.+)$")
  if path then
    file, item = path, nil
  elseif raw:match("^@@") then
    item = raw:match("^@@[^@]*@@ ?(.*)$") or ""
  elseif file and raw:match("^[+-]") and not raw:match("^%-%-%-") then
    for _, rule in ipairs(RULES) do
      if rule.file == file and (not rule.under or (item or ""):match(rule.under)) and matches(rule, raw) then
        remind(rule)
      end
    end
  end
end

if #order > 0 then
  io.stderr:write("pre-commit: docs reminder (not blocking). Your change touches things the docs list by hand:\n")
  for _, row in ipairs(order) do
    io.stderr:write("  ", row, ":\n")
    for _, page in ipairs(reminders[row]) do io.stderr:write("    ", page, "\n") end
  end
  io.stderr:write("  The full lookup is docs/how-to/update-the-docs.md. If the docs already say this, carry on.\n")
end
