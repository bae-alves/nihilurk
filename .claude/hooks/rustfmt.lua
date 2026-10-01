-- PostToolUse hook for Write|Edit: run rustfmt on the .rs file Claude just
-- changed. Claude Code passes the tool call as JSON on stdin.
--
-- Always exits 0. A formatting hook that can fail a tool call costs more than
-- it saves, so a bad payload or a rustfmt error is dropped silently.
--
-- Lua has no JSON in its standard library, and the one field wanted is a
-- string, so it is read by hand. `\uXXXX` escapes are not decoded; Claude Code
-- writes paths as raw UTF-8.

local input = io.read("a") or ""

local function string_after(s, key)
  local _, e = s:find('"' .. key .. '"%s*:%s*"')
  if not e then return nil end
  local out, i = {}, e + 1
  while i <= #s do
    local c = s:sub(i, i)
    if c == '"' then return table.concat(out) end
    if c == "\\" then
      i = i + 1
      local n = s:sub(i, i)
      c = ({ n = "\n", t = "\t", r = "\r" })[n] or n -- \/ \\ \" stand for themselves
    end
    out[#out + 1] = c
    i = i + 1
  end
end

local path = string_after(input, "file_path")
if path and path:match("%.rs$") then
  local quoted = "'" .. path:gsub("'", "'\\''") .. "'"
  os.execute("rustfmt --edition 2024 " .. quoted .. " >/dev/null 2>&1")
end
