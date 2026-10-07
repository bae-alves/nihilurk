-- lua .githooks/stub_missing_translations_test.lua
--
-- stub_missing_translations.lua must see `pub const fn` on both sides: en.rs
-- declares many, and the locales define them as `pub const fn` too. Seeing
-- only one side stubs a translated function twice or never stubs a new one.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local stub = dofile(here .. "/stub_missing_translations.lua")

local en = "pub fn plain() -> &'static str {\n    \"a\"\n}\n"
    .. "\npub const fn fixed() -> &'static str {\n    \"b\"\n}\n"
    .. "\npub const fn fresh(n: i32) -> String {\n    format!(\"{n}\")\n}\n"

local sigs, order = stub.en_signatures(en)
assert(#order == 3, "expected 3 signatures, got " .. #order)
assert(order[2] == "fixed" and sigs.fixed.const, "const fn is read, in en.rs order")
assert(not sigs.plain.const)

local locale = "pub fn plain() -> &'static str {\n    \"x\"\n}\n"
    .. "pub const fn fixed() -> &'static str {\n    \"y\"\n}\n"
local covered = stub.covered_names(locale)
assert(covered.plain and covered.fixed, "a locale's pub const fn counts as covered")
assert(not covered.fresh)

local text = stub.stub_for("fresh", sigs.fresh.params, sigs.fresh.ret, sigs.fresh.const)
assert(text:find("pub const fn fresh(n: i32) -> String {", 1, true), "stub keeps const")
print("ok")
