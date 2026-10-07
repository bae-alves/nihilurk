-- lua release/bump_test.lua
--
-- Runs release/bump.lua inside a throwaway git repo shaped like this one: a
-- small workspace, a man page, a PKGBUILD. `--no-cargo` skips the cargo steps
-- (lock refresh, tests, publish dry run) so the test needs no toolchain, and
-- still commits and tags, so the whole local flow is covered.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local bump = io.popen("cd '" .. here .. "' && pwd"):read("l") .. "/bump.lua"

local function sh(cmd)
  local p = assert(io.popen(cmd .. " 2>&1", "r"))
  local out = p:read("a")
  local _, _, code = p:close()
  return out, code
end

local dir = os.tmpname()
os.remove(dir)
assert(os.execute("mkdir -p '" .. dir .. "'"))
local function in_repo(cmd) return sh("cd '" .. dir .. "' && " .. cmd) end

local function write(path, text)
  assert(os.execute("mkdir -p '" .. dir .. "/" .. (path:match("^(.*)/[^/]*$") or ".") .. "'"))
  local f = assert(io.open(dir .. "/" .. path, "w"))
  f:write(text)
  f:close()
end
local function read(path)
  local f = assert(io.open(dir .. "/" .. path))
  local s = f:read("a")
  f:close()
  return s
end

-- The same shapes as the real manifests: the versions are not in lockstep
-- yet, a pin sits on a `path` line, and there are lines that look like a
-- version without being one (`rust-version`, third-party `version = "1"`).
local function fixture()
  write("Cargo.toml", '[workspace]\nresolver = "3"\nmembers = [\n    "compat",\n    "engine",\n    "models",\n    "particle-core",\n    "strings"\n]\n')
  write("compat/Cargo.toml", '[package]\nname = "nihilurk-compat"\nversion = "0.1.0"\npublish = false\n')
  write("engine/Cargo.toml", [[
[package]
name = "nihilurk"
version = "0.1.2"
rust-version = "1.91"

[dependencies]
models = { package = "nihilurk-models", path = "../models", version = "0.1.2", default-features = false }
strings = { package = "nihilurk-strings", path = "../strings", version = "0.1.0", default-features = false }
crossterm = "0.29.0"
]])
  write("models/Cargo.toml", [[
[package]
name = "nihilurk-models"
version = "0.1.2"
rust-version = "1.91"

[dependencies]
strings = { package = "nihilurk-strings", path = "../strings", version = "0.1.0", default-features = false }
particle-core = { package = "nihilurk-particle-core", path = "../particle-core", version = "0.1.0", features = ["std"] }
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
syn = { version = "2", features = ["full"] }
]])
  write("strings/Cargo.toml", '[package]\nname = "nihilurk-strings"\nversion = "0.1.0"\nrust-version = "1.85"\n')
  write("particle-core/Cargo.toml", '[package]\nname = "nihilurk-particle-core"\nversion = "0.1.0"\nrust-version = "1.85"\nstd = "x"')
  write("doc/nihilurk.6", '.TH NIHILURK 6 "October 2026" "nihilurk 0.1.0" "Games"\n.SH NAME\nnihilurk 0.1.2 is not a header\n')
  write("aur/PKGBUILD", "pkgname=nihilurk\npkgver=0.1.2\npkgrel=3\nsha256sums=('abc')\n")
  in_repo("git init -q -b master && git config user.email t@t && git config user.name t && git add -A && git commit -qm init")
end

local function reset()
  os.execute("rm -rf '" .. dir .. "' && mkdir -p '" .. dir .. "'")
  fixture()
end

local function bump_cmd(args) return in_repo("lua '" .. bump .. "' " .. args) end
local function has(s, needle) return s:find(needle, 1, true) ~= nil end

-- 1. `patch` moves every published crate to one number past the highest, and
--    every pin to the same number. Nothing else that looks like a version moves.
reset()
local out, code = bump_cmd("patch --no-cargo")
assert(code == 0, "patch should succeed, got: " .. out)
local engine = read("engine/Cargo.toml")
assert(has(engine, '\nversion = "0.1.3"\n'), "engine version")
assert(has(engine, 'path = "../models", version = "0.1.3"'), "engine's models pin")
assert(has(engine, 'path = "../strings", version = "0.1.3"'), "engine's strings pin")
assert(has(engine, 'rust-version = "1.91"'), "rust-version must not move")
assert(has(engine, 'crossterm = "0.29.0"'), "a third-party dependency must not move")
local models = read("models/Cargo.toml")
assert(has(models, '\nversion = "0.1.3"\n'), "models version")
assert(has(models, 'path = "../particle-core", version = "0.1.3"'), "models' particle-core pin")
assert(has(models, 'serde = { version = "1"'), "third-party version = 1 must not move")
assert(has(models, 'syn = { version = "2"'), "third-party version = 2 must not move")
assert(has(read("strings/Cargo.toml"), '\nversion = "0.1.3"\n'), "strings joins lockstep")
assert(has(read("particle-core/Cargo.toml"), '\nversion = "0.1.3"\n'), "particle-core joins lockstep")
assert(read("particle-core/Cargo.toml"):sub(-1) == '"', "a file with no final newline must not gain one")
assert(has(read("compat/Cargo.toml"), 'version = "0.1.0"'), "the test rig is not published and stays put")

-- 2. The man page header and the PKGBUILD follow.
local man = read("doc/nihilurk.6")
assert(has(man, '.TH NIHILURK 6 "' .. os.date("%B %Y") .. '" "nihilurk 0.1.3" "Games"'), "man header: " .. man)
assert(has(man, "nihilurk 0.1.2 is not a header"), "only the .TH line changes")
local pkg = read("aur/PKGBUILD")
assert(has(pkg, "pkgver=0.1.3\n") and has(pkg, "pkgrel=1\n"), "pkgver bumped, pkgrel reset")
assert(has(pkg, "sha256sums=('abc')"), "the sha is not this script's job")

-- 3. It commits and tags, leaves a clean tree, and says what to push. It does
--    not push.
assert(in_repo("git log -1 --format=%s") == "Release v0.1.3\n", "commit subject")
assert(has(in_repo("git tag"), "v0.1.3"), "tag")
assert(in_repo("git status --porcelain") == "", "clean tree after the release commit")
assert(has(out, "git push origin master v0.1.3"), "it should print the push command")

-- 4. `minor`, `major` and an explicit number.
reset()
bump_cmd("minor --no-cargo")
assert(has(read("engine/Cargo.toml"), '\nversion = "0.2.0"\n'), "minor")
reset()
bump_cmd("major --no-cargo")
assert(has(read("engine/Cargo.toml"), '\nversion = "1.0.0"\n'), "major")
reset()
bump_cmd("0.4.7 --no-cargo")
assert(has(read("models/Cargo.toml"), '\nversion = "0.4.7"\n'), "explicit number")

-- 5. --dry-run shows the change and touches nothing.
reset()
out, code = bump_cmd("patch --dry-run")
assert(code == 0 and has(out, "0.1.3"), "dry run should show the new number")
assert(has(read("engine/Cargo.toml"), '\nversion = "0.1.2"\n'), "dry run must not edit files")
assert(in_repo("git tag") == "", "dry run must not tag")

-- 6. It refuses, and changes nothing, when the state is wrong.
local function refuses(why, args, expect)
  local o, c = bump_cmd(args)
  assert(c == 1, why .. ": should exit 1, got " .. tostring(c) .. "\n" .. o)
  assert(has(o, expect), why .. ": should say '" .. expect .. "', got: " .. o)
  assert(has(read("engine/Cargo.toml"), '\nversion = "0.1.2"\n'), why .. ": must not edit files")
end
reset()
write("stray.txt", "x")
refuses("dirty tree", "patch --no-cargo", "uncommitted")
reset()
in_repo("git checkout -q -b other")
refuses("wrong branch", "patch --no-cargo", "master")
reset()
in_repo("git tag v0.1.3")
refuses("existing tag", "patch --no-cargo", "already exists")
reset()
refuses("same version", "0.1.2 --no-cargo", "greater")
refuses("older version", "0.0.9 --no-cargo", "greater")
refuses("not a version", "banana --no-cargo", "usage")

-- 7. A changed `SAVE_VERSION` since the last tag means old saves stop loading:
--    that is a minor bump at least. No tag, no constant, or no change means no
--    opinion. A tag from before the constant existed counts as version 0.
local function save_file(n)
  return "pub const SAVE_VERSION: u16 = " .. n .. ";\n"
end
local function tagged_with(n, then_n)
  reset()
  if n then
    write("models/src/saveload.rs", save_file(n))
    in_repo("git add -A && git commit -qm save")
  end
  in_repo("git tag v0.1.2")
  if then_n then
    write("models/src/saveload.rs", save_file(then_n))
    in_repo("git add -A && git commit -qm bump-save")
  end
end
local function refuses_save(args)
  local o, c = bump_cmd(args)
  assert(c == 1, args .. ": should exit 1, got " .. tostring(c) .. "\n" .. o)
  assert(has(o, "SAVE_VERSION"), args .. ": should name SAVE_VERSION, got: " .. o)
  assert(has(read("engine/Cargo.toml"), '\nversion = "0.1.2"\n'), "must not edit files")
end

tagged_with(1, 2)
refuses_save("patch --no-cargo")
refuses_save("0.1.9 --no-cargo")
out, code = bump_cmd("minor --no-cargo")
assert(code == 0, "minor passes a save change: " .. out)
tagged_with(1, 2)
out, code = bump_cmd("major --no-cargo")
assert(code == 0, "major passes a save change: " .. out)
tagged_with(1, 2)
out, code = bump_cmd("0.2.0 --no-cargo")
assert(code == 0, "an explicit minor passes a save change: " .. out)

tagged_with(1, 1)
out, code = bump_cmd("patch --no-cargo")
assert(code == 0, "an unchanged constant lets patch through: " .. out)

tagged_with(nil, 1)
refuses_save("patch --no-cargo")

reset()
write("models/src/saveload.rs", save_file(3))
in_repo("git add -A && git commit -qm save")
out, code = bump_cmd("patch --no-cargo")
assert(code == 0, "no tag, no opinion: " .. out)

os.execute("rm -rf '" .. dir .. "'")
print("ok  bump.lua")
