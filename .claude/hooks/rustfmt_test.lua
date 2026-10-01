-- lua .claude/hooks/rustfmt_test.lua
--
-- Checks rustfmt.lua the way Claude Code calls it: hook JSON on stdin, exit
-- code back. The two things that matter are that it formats a .rs file and
-- that it never fails a tool call, whatever it is fed.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local hook = here .. "/rustfmt.lua"

local function run(stdin)
  local p = assert(io.popen("lua '" .. hook .. "' >/dev/null 2>&1", "w"))
  p:write(stdin)
  local _, _, code = p:close()
  return code
end

local function write(path, text)
  local f = assert(io.open(path, "w"))
  f:write(text)
  f:close()
end

local function read(path)
  local f = assert(io.open(path, "r"))
  local text = f:read("a")
  f:close()
  return text
end

local function json(path)
  return '{"tool_name":"Edit","tool_input":{"file_path":"' .. path .. '","old_string":"x"}}'
end

local dir = os.tmpname()
os.remove(dir)
assert(os.execute("mkdir -p '" .. dir .. "'"))

local ugly = "fn main(){let x=1;}\n"
local tidy = "fn main() {\n    let x = 1;\n}\n"

-- 1. A .rs file gets formatted.
write(dir .. "/a.rs", ugly)
assert(run(json(dir .. "/a.rs")) == 0, "exit 0 on a .rs file")
assert(read(dir .. "/a.rs") == tidy, "a.rs should be formatted")

-- 2. A path with a space and a quote survives the shell.
write(dir .. "/it's here.rs", ugly)
assert(run(json(dir .. "/it's here.rs")) == 0)
assert(read(dir .. "/it's here.rs") == tidy, "quoted path should be formatted")

-- 3. JSON escapes the slash as \/ sometimes; the path still resolves.
write(dir .. "/b.rs", ugly)
assert(run(json((dir .. "/b.rs"):gsub("/", "\\/"))) == 0)
assert(read(dir .. "/b.rs") == tidy, "an escaped path should be formatted")

-- 4. Anything that is not .rs is left alone.
write(dir .. "/c.txt", ugly)
assert(run(json(dir .. "/c.txt")) == 0)
assert(read(dir .. "/c.txt") == ugly, "c.txt must not be touched")

-- 5. It never fails the tool call: garbage, no path, a .rs that is gone.
assert(run("not json") == 0, "exit 0 on garbage")
assert(run("") == 0, "exit 0 on empty input")
assert(run('{"tool_input":{}}') == 0, "exit 0 with no file_path")
assert(run(json(dir .. "/gone.rs")) == 0, "exit 0 on a missing file")

os.execute("rm -rf '" .. dir .. "'")
print("ok  rustfmt.lua")
