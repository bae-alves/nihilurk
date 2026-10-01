-- Refuses a commit that stages Python. Called first by .githooks/pre-commit.
--
-- `--tree` checks every tracked file instead of the staged ones. CI uses it:
-- a contributor's clone may never have enabled the pre-commit hook.
--
-- Caught by name (.py, .pyw) and by shebang, so a script with no extension and
-- a `#!/usr/bin/env python3` line cannot slip through. Only files are
-- checked, never their text: a doc may say the word.
--
-- nihilurk is Rust, bash and Lua. Scripts and hooks are bash or Lua.

local MESSAGE = "No python. Lua is to be used"

local function shell_quote(s)
  return "'" .. s:gsub("'", "'\\''") .. "'"
end

local list = arg[1] == "--tree" and "git ls-files -z"
  or "git diff --cached --name-only --diff-filter=ACM -z"
local staged = io.popen(list)
local names = staged:read("a")
staged:close()

local found = {}
for name in names:gmatch("[^\0]+") do
  local hit = name:match("%.pyw?$") ~= nil
  if not hit then
    -- The staged blob, not the working copy: that is what would be committed.
    local blob = io.popen("git show :" .. shell_quote(name) .. " 2>/dev/null | head -c 200")
    local first = blob:read("l") or ""
    blob:close()
    hit = first:match("^#!.*python") ~= nil
  end
  if hit then found[#found + 1] = name end
end

if #found > 0 then
  io.stderr:write(MESSAGE, "\n")
  for _, name in ipairs(found) do io.stderr:write("  ", name, "\n") end
  os.exit(1)
end
