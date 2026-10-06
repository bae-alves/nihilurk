-- lua .githooks/tag_english_placeholders_test.lua
--
-- tag_english_placeholders.lua tags only the items en.rs gained since HEAD,
-- above their doc comments, and leaves a human's untagged edit alone.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local tagger = dofile(here .. "/tag_english_placeholders.lua")
local TAG = "// TODO: placeholder English; needs a human's pass."

local old = "// a human wrote this\npub fn kept() -> &'static str {\n    \"x\"\n}\n"
local new = old
    .. "\n/// Docs.\n#[allow(unused)]\npub fn added(n: i32) -> String {\n    format!(\"{n}\")\n}\n"
    .. "\npub const fn added_const() -> &'static str {\n    \"y\"\n}\n"
    .. "\npub const ADDED_LIST: [&str; 1] = [\n    \"z\",\n];\n"
    .. "\npub fn content_name(id: &str) -> &str {\n    id\n}\n"

local out, n = tagger.tag(new, old)
assert(n == 3, "expected 3 tags, got " .. n)
assert(out:find(TAG .. "\n/// Docs.\n#[allow(unused)]\npub fn added", 1, true), "tag goes above docs and attributes")
assert(out:find(TAG .. "\npub const fn added_const", 1, true))
assert(out:find(TAG .. "\npub const ADDED_LIST", 1, true))
assert(not out:find(TAG .. "\npub fn kept", 1, true), "existing untagged item stays untagged")
assert(not out:find(TAG .. "\npub fn content_name", 1, true), "content_name is not a sentence")

local again, m = tagger.tag(out, old)
assert(m == 0 and again == out, "second run changes nothing")

local fresh, k = tagger.tag(old, "")
assert(k == 1 and fresh:find(TAG, 1, true), "no HEAD copy: everything is new")
print("ok")
