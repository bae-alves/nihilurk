-- lua .githooks/no_python_test.lua
--
-- Runs no_python.lua, and the real pre-commit, inside a throwaway git repo
-- with different things staged. The hook refuses Python by file name and by
-- shebang; it must leave prose that merely mentions the word alone.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local root = io.popen("cd '" .. here .. "' && pwd"):read("l")
local hooks = root

local function sh(cmd)
  local p = assert(io.popen(cmd .. " 2>&1", "r"))
  local out = p:read("a")
  local _, _, code = p:close()
  return out, code
end

local dir = os.tmpname()
os.remove(dir)
assert(os.execute("mkdir -p '" .. dir .. "/.githooks'"))
assert(os.execute("cp '" .. hooks .. "/pre-commit' '" .. hooks .. "/no_python.lua' '" .. dir .. "/.githooks/'"))
sh("cd '" .. dir .. "' && git init -q && git config user.email t@t && git config user.name t")

local function stage(name, text)
  local f = assert(io.open(dir .. "/" .. name, "w"))
  f:write(text)
  f:close()
  sh("cd '" .. dir .. "' && git add -f -- '" .. name .. "'")
end

local function unstage_all()
  sh("cd '" .. dir .. "' && git rm -rq --cached -f . 2>/dev/null; true")
end

local function check(flag)
  return sh("cd '" .. dir .. "' && lua .githooks/no_python.lua " .. (flag or ""))
end

local MESSAGE = "No python. Lua is to be used"

-- 1. A .py file is refused, with the message first and the file named.
stage("tool.py", "print('hi')\n")
local out, code = check()
assert(code == 1, "a .py file should refuse, got " .. tostring(code))
assert(out:match("^" .. MESSAGE), "message first, got: " .. out)
assert(out:find("tool.py", 1, true), "the file should be named")
unstage_all()

-- 2. An extensionless script with a python shebang is refused.
stage("runme", "#!/usr/bin/env python3\nprint('hi')\n")
out, code = check()
assert(code == 1, "a python shebang should refuse")
assert(out:find("runme", 1, true))
unstage_all()
stage("runme2", "#!/usr/bin/python\n")
assert(select(2, check()) == 1, "/usr/bin/python should refuse too")
unstage_all()

-- 3. Lua and bash pass, and so does prose that says the word.
stage("ok.lua", "#!/usr/bin/env lua\nprint('hi')\n")
stage("ok.sh", "#!/usr/bin/env bash\necho hi\n")
stage("notes.md", "No python here, but the word is in the text. python python.\n")
out, code = check()
assert(code == 0, "lua, bash and prose should pass, got: " .. out)
unstage_all()

-- 3b. --tree checks everything tracked, not only what is staged. CI uses it,
-- because a contributor's clone may never have run the pre-commit hook.
stage("old.py", "print(1)\n")
sh("cd '" .. dir .. "' && git commit -qm old --no-verify")
assert(select(2, check()) == 0, "nothing staged, so the staged check passes")
out, code = check("--tree")
assert(code == 1, "--tree should refuse a committed .py file")
assert(out:match("^" .. MESSAGE) and out:find("old.py", 1, true), "message, then the file")
sh("cd '" .. dir .. "' && git rm -q -f old.py && git commit -qm drop --no-verify")
assert(select(2, check("--tree")) == 0, "--tree passes once the python is gone")

-- 4. The real pre-commit refuses, and runs on the same staged files.
stage("tool.py", "print('hi')\n")
out, code = sh("cd '" .. dir .. "' && bash .githooks/pre-commit")
assert(code == 1, "pre-commit should refuse a .py file")
assert(out:find(MESSAGE, 1, true), "pre-commit should print the message")
unstage_all()
stage("ok.lua", "print('hi')\n")
out, code = sh("cd '" .. dir .. "' && bash .githooks/pre-commit")
assert(code == 0, "pre-commit should pass a .lua file, got: " .. out)

os.execute("rm -rf '" .. dir .. "'")
print("ok  no_python.lua")
