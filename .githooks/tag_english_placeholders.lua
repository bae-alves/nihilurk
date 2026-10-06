#!/usr/bin/env lua
--
-- Tags every new English string as a placeholder.
--
-- Called from `.githooks/pre-commit` whenever `strings/src/en.rs` is staged.
-- English is placeholder text until a human replaces it, so any `pub fn` or
-- `pub const` the file gained since HEAD gets `// TODO: placeholder English;
-- needs a human's pass.` above its doc comments. Items HEAD already has are
-- left alone: a human who rewrites a string deletes its tag, and this must
-- not put it back. `content_name` is an id lookup, not a sentence, so it
-- never gets one. Prints the path when it changed the file.

local TAG = "// TODO: placeholder English; needs a human's pass."
local PATH = "strings/src/en.rs"

local function item_name(line)
    return line:match("^pub fn ([%w_]+)")
        or line:match("^pub const fn ([%w_]+)")
        or line:match("^pub const ([A-Z][A-Z0-9_]*)")
end

local M = {}

--- Returns `text` with the tag added to each item `old_text` lacks, and how
--- many it added.
function M.tag(text, old_text)
    local known = {}
    for line in old_text:gmatch("[^\n]+") do
        local name = item_name(line)
        if name then
            known[name] = true
        end
    end

    local out, n = {}, 0
    for line in (text:gsub("\n?$", "\n")):gmatch("(.-)\n") do
        local name = item_name(line)
        if name and name ~= "content_name" and not known[name] then
            local j = #out
            while j > 0 and (out[j]:match("^///") or out[j]:match("^#%[")) do
                j = j - 1
            end
            if out[j] ~= TAG then
                table.insert(out, j + 1, TAG)
                n = n + 1
            end
        end
        out[#out + 1] = line
    end
    return table.concat(out, "\n") .. "\n", n
end

if arg and arg[0] and arg[0]:match("tag_english_placeholders%.lua$") then
    local f = assert(io.open(PATH, "r"), "can't open " .. PATH)
    local text = f:read("a")
    f:close()
    -- No HEAD copy (first commit, or the file is new): everything is new.
    local p = io.popen("git show HEAD:" .. PATH .. " 2>/dev/null")
    local old = p:read("a")
    p:close()
    local new, n = M.tag(text, old)
    if n > 0 then
        f = assert(io.open(PATH, "w"))
        f:write(new)
        f:close()
        print(PATH)
    end
end

return M
