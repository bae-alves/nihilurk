-- release/bump.lua -- everything a release edits by hand, in one command.
--
--   lua release/bump.lua <X.Y.Z | patch | minor | major> [--no-cargo] [--dry-run]
--
-- All four published crates move to one version (lockstep), and every `path`
-- dependency pin moves with them. The man page header, the PKGBUILD's
-- pkgver and the README's download links follow. Then, unless --no-cargo: Cargo.lock is refreshed, the tests
-- run, and a publish dry run proves the packages build. Then it commits
-- `Release vX.Y.Z` and tags it.
--
-- It never pushes. Pushing the tag is what starts the release, so it stays a
-- decision made at the keyboard: the last line printed is the command.
--
--   --no-cargo   skip the cargo steps (the test uses this; so can you, if you
--                have just run them yourself)
--   --dry-run    show what would change and change nothing
--
-- Versions are X.Y.Z only; there is no pre-release syntax. The recipe, and what
-- CI does with the tag, is docs/how-to/cut-a-release.md.

local USAGE = "usage: lua release/bump.lua <X.Y.Z | patch | minor | major> [--no-cargo] [--dry-run]"

local function die(msg)
  io.stderr:write(msg, "\n")
  os.exit(1)
end

local function q(s) return "'" .. s:gsub("'", "'\\''") .. "'" end
local function trim(s) return (s:gsub("%s+$", "")) end

local function capture(cmd)
  local p = assert(io.popen(cmd .. " 2>&1", "r"))
  local out = p:read("a")
  local ok = p:close()
  return out, ok
end

-- arguments
local target, no_cargo, dry_run
for _, a in ipairs(arg) do
  if a == "--no-cargo" then no_cargo = true
  elseif a == "--dry-run" then dry_run = true
  elseif not target then target = a
  else die(USAGE) end
end
if not target then die(USAGE) end

local top, in_repo = capture("git rev-parse --show-toplevel")
if not in_repo then die("not inside a git repository") end
local root = trim(top)

local function git(args) return capture("git -C " .. q(root) .. " " .. args) end
local function path(p) return root .. "/" .. p end

local function read(p)
  local f = assert(io.open(path(p)), "cannot read " .. p)
  local s = f:read("a")
  f:close()
  return s
end

local function write(p, s)
  local f = assert(io.open(path(p), "w"))
  f:write(s)
  f:close()
end

-- The published crates: every workspace member that is not `publish = false`.
local manifest = read("Cargo.toml")
local members = assert(manifest:match("\nmembers%s*=%s*%[(.-)%]"), "no `members` in Cargo.toml")
local crates = {}
for dir in members:gmatch('"([^"]+)"') do
  if not read(dir .. "/Cargo.toml"):find("publish = false", 1, true) then
    crates[#crates + 1] = dir
  end
end

local function parse(v)
  local a, b, c = v:match("^(%d+)%.(%d+)%.(%d+)$")
  if not a then return nil end
  return { tonumber(a), tonumber(b), tonumber(c) }
end

local function newer(a, b) -- is a greater than b
  for i = 1, 3 do
    if a[i] ~= b[i] then return a[i] > b[i] end
  end
  return false
end

local highest
for _, dir in ipairs(crates) do
  local v = parse(read(dir .. "/Cargo.toml"):match('\nversion = "([^"]+)"') or "")
  assert(v, dir .. "/Cargo.toml has no X.Y.Z version")
  if not highest or newer(v, highest) then highest = v end
end

local new
if target == "patch" then new = { highest[1], highest[2], highest[3] + 1 }
elseif target == "minor" then new = { highest[1], highest[2] + 1, 0 }
elseif target == "major" then new = { highest[1] + 1, 0, 0 }
else new = parse(target) end
if not new then die(USAGE) end
local version = table.concat(new, ".")
local tag = "v" .. version

-- refusals: nothing is edited until all of these pass
if git("status --porcelain") ~= "" then
  die("uncommitted changes in the working tree. Commit or stash them first.")
end
if trim(git("rev-parse --abbrev-ref HEAD")) ~= "master" then
  die("not on master. A release is cut from master.")
end
if select(2, git("rev-parse -q --verify refs/tags/" .. tag)) then
  die("tag " .. tag .. " already exists")
end
if not newer(new, highest) then
  die(version .. " is not greater than the current " .. table.concat(highest, "."))
end

-- A changed SAVE_VERSION since the last tag means old saves stop loading, which
-- is a minor bump at least. No tag, or no constant in the working tree, means no
-- opinion; a tag from before the constant existed counts as version 0.
local function save_version(text)
  return tonumber((text or ""):match("SAVE_VERSION:%s*u%d+%s*=%s*(%d+)"))
end
local SAVE_FILE = "models/src/saveload.rs"
local last_tag, has_tag = git("describe --tags --abbrev=0")
local f = io.open(path(SAVE_FILE))
local now = f and save_version(f:read("a"))
if f then f:close() end
if has_tag and now then
  last_tag = trim(last_tag)
  local old, had_file = git("show " .. last_tag .. ":" .. SAVE_FILE)
  local was = had_file and save_version(old) or 0
  if was ~= now and new[1] == highest[1] and new[2] == highest[2] then
    die(string.format("SAVE_VERSION went from %d to %d since %s: old saves stop loading, " ..
      "so this is a minor bump at least. Nothing was edited.", was, now, last_tag))
  end
end

-- the edits, as text in, text out
local function count_one(s, n)
  assert(n == 1, "expected to change exactly one place, changed " .. n)
  return s
end

local function crate_manifest(text)
  -- the package version, then every path pin (a `version` on a `path = "../` line)
  local out, n = text:gsub('(\nversion = ")[^"]*(")', "%1" .. version .. "%2", 1)
  count_one(out, n)
  -- per line, so the bytes between lines (and a missing final newline) survive
  return (out:gsub("[^\n]+", function(line)
    if line:find('path = "../', 1, true) then
      return (line:gsub('(version = ")[^"]*(")', "%1" .. version .. "%2"))
    end
  end))
end

local function man_page(text)
  local out, n = text:gsub('(%.TH NIHILURK 6 )"[^"]*" "nihilurk [^"]*"',
    '%1"' .. os.date("%B %Y") .. '" "nihilurk ' .. version .. '"')
  return count_one(out, n)
end

local function pkgbuild(text)
  local out, n = text:gsub("(\npkgver=)[^\n]*", "%1" .. version)
  count_one(out, n)
  out, n = out:gsub("(\npkgrel=)[^\n]*", "%11")
  return count_one(out, n)
end

-- The download links and the "(vX.Y.Z)" in the line above them. A link that
-- stays behind would send a reader to the old release, so a README with
-- neither is an error and not a quiet no-op.
local function readme(text)
  local out, links = text:gsub("(/releases/download/)v%d+%.%d+%.%d+(/nihilurk%-)%d+%.%d+%.%d+(%-)",
    "%1" .. tag .. "%2" .. version .. "%3")
  assert(links > 0, "README.md has no download links to move")
  local n
  out, n = out:gsub("(prebuilt game %()v%d+%.%d+%.%d+(%))", "%1" .. tag .. "%2")
  return count_one(out, n)
end

local edits = {}
for _, dir in ipairs(crates) do
  edits[#edits + 1] = { dir .. "/Cargo.toml", crate_manifest }
end
edits[#edits + 1] = { "doc/nihilurk.6", man_page }
edits[#edits + 1] = { "aur/PKGBUILD", pkgbuild }
edits[#edits + 1] = { "README.md", readme }

local changed = {}
for _, e in ipairs(edits) do
  local old = read(e[1])
  local text = e[2](old)
  if text ~= old then changed[#changed + 1] = { e[1], old, text } end
end

local function lines_of(s)
  local t = {}
  for line in (s .. "\n"):gmatch("(.-)\n") do t[#t + 1] = line end
  return t
end

print(string.format("%s -> %s", table.concat(highest, "."), version))
for _, c in ipairs(changed) do
  print("  " .. c[1])
  local a, b = lines_of(c[2]), lines_of(c[3])
  for i = 1, math.max(#a, #b) do
    if a[i] ~= b[i] then
      print("    - " .. (a[i] or ""))
      print("    + " .. (b[i] or ""))
    end
  end
end

if dry_run then
  print("dry run: nothing changed")
  os.exit(0)
end

for _, c in ipairs(changed) do write(c[1], c[3]) end

if not no_cargo then
  local steps = {
    "cargo update --workspace",
    "cargo test --locked --workspace --exclude nihilurk-compat",
    "cargo publish --workspace --dry-run --allow-dirty",
  }
  for _, step in ipairs(steps) do
    print("\n$ " .. step)
    if not os.execute("cd " .. q(root) .. " && " .. step) then
      die("\n`" .. step .. "` failed. The edits are still in the working tree; " ..
        "`git checkout .` undoes them.")
    end
  end
end

local _, committed = git("commit -aqm " .. q("Release " .. tag))
if not committed then
  die("the commit was refused (a hook?). The edits are still in the working tree; " ..
    "`git checkout .` undoes them.")
end
git("tag " .. tag)

print("\nCommitted and tagged " .. tag .. ". Nothing is pushed. To release:\n")
print("    git push origin master " .. tag)
