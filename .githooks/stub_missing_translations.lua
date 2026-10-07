#!/usr/bin/env lua
--
-- Auto-stubs any `strings::en` function a locale hasn't caught up with yet.
--
-- Called from `.githooks/pre-commit`, never by hand. `en.rs` is the only
-- module with real content (see `strings/src/lib.rs`'s doc comment); `pt.rs`
-- and `es.rs` are expected to lag behind it, function by function, as they
-- get translated. Before this script existed, that lag was the committer's
-- problem: a new `en.rs` function with no `pt`/`es` line failed the build
-- and the commit with it, whether or not the committer reads either
-- language.
--
-- Instead: for every `pub fn` in `en.rs` that a locale file neither defines
-- itself nor re-exports from `en` (see the `pub use super::en::{...}` block
-- at the top of `pt.rs`/`es.rs` -- that one is a deliberate, permanent
-- English-only list, not a translation debt, and covering a name there
-- counts as covering it here too), this appends a stub with the same
-- signature that returns an empty value, marked `// TODO: translate.`. A
-- blank string reads as obviously wrong in play -- which is the point: it
-- is a louder flag than quietly showing English text would be, so it does
-- not get mistaken for a finished translation. Re-run any time; a name this
-- has already stubbed (or a human has since translated for real) is a
-- `pub fn` in the file already, so it is never stubbed twice.
--
-- `ht.rs` is not touched -- it re-exports `en` wholesale on purpose (see
-- its own doc comment) and never gains per-function overrides.

local script_dir = (arg[0] or ""):match("(.*/)") or "./"
local root = script_dir .. "../"
local strings_src = root .. "strings/src/"

local function read_file(path)
    local f = assert(io.open(path, "r"), "can't open " .. path)
    local content = f:read("a")
    f:close()
    return content
end

local function write_file(path, content)
    local f = assert(io.open(path, "w"), "can't open " .. path .. " for writing")
    f:write(content)
    f:close()
end

local function trim(s)
    return s:match("^%s*(.-)%s*$")
end

--- name -> {params = ..., ret = ...}, plus `order` listing names as `en.rs`
--- declares them, so stubs land in a stable, reviewable order.
local function en_signatures(en_text)
    local sigs, order = {}, {}
    local plain = en_text:gsub("pub%s+const%s+fn", "pub fn")
    for name, params, ret in
        plain:gmatch("pub fn%s+([%w_]+)%s*%(([^%)]*)%)%s*%-%>%s*([^{]*){")
    do
        if not sigs[name] then
            table.insert(order, name)
        end
        sigs[name] = {
            params = trim(params),
            ret = trim(ret),
            const = en_text:find("pub%s+const%s+fn%s+" .. name .. "%s*%(") ~= nil,
        }
    end
    return sigs, order
end

--- Every name a locale file already accounts for: a `pub fn` of its own, or
--- one named (singly or in a `{...}` block) in a `pub use super::en::...`
--- re-export.
local function covered_names(locale_text)
    local covered = {}
    for line in locale_text:gmatch("[^\n]*") do
        local name = line:match("^pub fn%s+([%w_]+)")
            or line:match("^pub const fn%s+([%w_]+)")
        if name then
            covered[name] = true
        end
    end
    for name in locale_text:gmatch("pub use super::en::([%w_]+);") do
        covered[name] = true
    end
    for block in locale_text:gmatch("pub use super::en::%{(.-)%};") do
        for name in block:gmatch("[%w_]+") do
            covered[name] = true
        end
    end
    return covered
end

local function empty_value_for(ret)
    if ret == "&'static str" or ret == "&str" then
        return '""'
    end
    if ret == "String" then
        return "String::new()"
    end
    local n = ret:match("^%[&'static str;%s*(%d+)%]$")
    if n then
        local parts = {}
        for _ = 1, tonumber(n) do
            table.insert(parts, '""')
        end
        return "[" .. table.concat(parts, ", ") .. "]"
    end
    -- Unknown shape: fail loudly rather than emit something that won't
    -- compile -- see the pre-commit check right after this script runs.
    error(
        "stub_missing_translations.lua: don't know an empty value for "
            .. "return type '"
            .. ret
            .. "' -- teach `empty_value_for` about it."
    )
end

local function stub_for(name, params, ret, const)
    return "\n// TODO: translate.\n"
        .. "#[allow(unused_variables)]\n"
        .. (const and "pub const fn " or "pub fn ")
        .. name
        .. "("
        .. params
        .. ") -> "
        .. ret
        .. " {\n    "
        .. empty_value_for(ret)
        .. "\n}\n"
end

--- Returns true if `locale` needed (and got) any stubs.
local function sync_locale(locale, sigs, order)
    local path = strings_src .. locale .. ".rs"
    local text = read_file(path)
    local covered = covered_names(text)

    local missing = {}
    for _, name in ipairs(order) do
        if not covered[name] then
            table.insert(missing, name)
        end
    end
    if #missing == 0 then
        return false
    end

    local addition = "\n// --- Auto-stubbed by .githooks/pre-commit: not yet translated. ---\n"
    for _, name in ipairs(missing) do
        local sig = sigs[name]
        addition = addition .. stub_for(name, sig.params, sig.ret, sig.const)
    end
    write_file(path, (text:gsub("\n+$", "")) .. "\n" .. addition)

    print(
        "stub_missing_translations.lua: stubbed "
            .. #missing
            .. " function(s) in strings/src/"
            .. locale
            .. ".rs: "
            .. table.concat(missing, ", ")
    )
    return true
end

local function main()
    local en_text = read_file(strings_src .. "en.rs")
    local sigs, order = en_signatures(en_text)

    local changed = {}
    for _, locale in ipairs({ "pt", "es" }) do
        if sync_locale(locale, sigs, order) then
            table.insert(changed, locale)
        end
    end
    for _, locale in ipairs(changed) do
        print("strings/src/" .. locale .. ".rs")
    end
end

if arg and arg[0] and arg[0]:match("stub_missing_translations%.lua$") then
    main()
end

return {
    en_signatures = en_signatures,
    covered_names = covered_names,
    stub_for = stub_for,
}
