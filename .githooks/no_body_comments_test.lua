-- lua .githooks/no_body_comments_test.lua
--
-- no_body_comments.lua finds comments inside function bodies. Comments above
-- an item, in a signature, or on their own line above a closure are not body
-- comments. It reads the staged blob, and only in engine, models and
-- particle-core.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local root = io.popen("cd '" .. here .. "' && pwd"):read("l")
local finder = dofile(root .. "/no_body_comments.lua")

local function lines_of(src)
    local found, err = finder.find(src)
    assert(found, err)
    local out = {}
    for _, c in ipairs(found) do out[#out + 1] = c.line end
    return out
end

local function expect(src, want, what)
    local got = table.concat(lines_of(src), ",")
    assert(got == want, what .. ": want lines [" .. want .. "], got [" .. got .. "]")
end

-- 1. A line comment in a body is found, with its line number.
expect("fn a() {\n    // why\n    let x = 1;\n}\n", "2", "own-line comment")

-- 2. A trailing comment on a line of code is found.
expect("fn a() {\n    let x = 1; // why\n}\n", "2", "trailing comment")

-- 3. A block comment in a body is found, nested ones included.
expect("fn a() {\n    /* a /* b */ c */\n    let x = 1;\n}\n", "2", "block comment")

-- 4. Comments outside bodies stay: file, item, doc, signature, attribute.
expect(
    "//! File.\n// Above.\n/// Doc.\n#[allow(dead_code)] // why\nfn a(\n    x: u8, // the x\n) -> u8 // out\n{\n    x\n}\n",
    "",
    "outside a body"
)

-- 5. impl, mod, static and macro_rules bodies are not function bodies.
expect(
    "impl S {\n    // one\n    fn a() {}\n}\nmod m {\n    // two\n}\nstatic T: [u8; 1] = [\n    1, // three\n];\nmacro_rules! m {\n    () => {\n        // four\n    };\n}\n",
    "",
    "non-function braces"
)

-- 6. Inside `mod tests`, a comment outside a test fn stays and one inside goes.
expect("mod tests {\n    // kept\n    #[test]\n    fn t() {\n        // gone\n    }\n}\n", "5", "tests")

-- 7. A nested fn or a closure body is still the outer body.
expect("fn a() {\n    fn b() {\n        // inner\n    }\n    let f = || {\n        // closure\n    };\n}\n", "3,6", "nesting")

-- 8. `//` and `/*` inside strings, raw strings, chars and lifetimes are not comments.
expect(
    "fn a<'a>(s: &'a str) -> &'a str {\n    let u = \"http://x\"; let r = r#\"a // b \"c\" /* d\"#; let c = '/'; let e = '\\'';\n    let b = b\"//\"; let m = 'é';\n    s\n}\n",
    "",
    "literals"
)

-- 9. A fn-pointer type in the signature does not end the signature early.
expect("fn a(f: fn(u8) -> u8, g: [u8; 3]) -> u8 {\n    // body\n    f(g[0])\n}\n", "2", "fn pointer")

-- 10. A trait method with no body does not swallow the next fn's body.
expect("trait T {\n    fn a(&self);\n    fn b(&self) {\n        // body\n    }\n}\n", "4", "bodiless fn")

-- 11. An own-line comment directly above a closure is not a body comment.
expect("fn a() {\n    // sorts by depth\n    let f = |x: u8| x + 1;\n}\n", "", "let closure")
expect("fn a(v: Vec<u8>) {\n    // keep the odd ones\n    let w: Vec<_> = v.iter().filter(|x| *x % 2 == 1).collect();\n}\n", "", "call closure")
expect("fn a() {\n    // off thread\n    std::thread::spawn(move || run());\n}\n", "", "move closure")
expect("fn a() {\n    // one\n    // two\n    let f = || 1;\n}\n", "", "comment block above")
expect("fn a() {\n    /* block */\n    let f = |x| x;\n}\n", "", "block above")

-- 11b. The shapes rustfmt gives a closure: a wrapped argument on its own
-- line, an array element, a struct field, a closure that follows an `=`.
expect("fn a() {\n    f(\n        x,\n        // doubles it\n        |item| item.value,\n    );\n}\n", "", "wrapped argument")
expect("fn a() {\n    f(\n        // no args\n        || g(1, 2),\n        3,\n    );\n}\n", "", "wrapped zero-arg")
expect("fn a() {\n    let r = [\n        // first\n        |x: u8| x + 1,\n        |x: u8| x + 2,\n    ];\n}\n", "", "array element")
expect("fn a() {\n    let r = Thing {\n        // the hook\n        callback: |item| item.value,\n    };\n}\n", "", "struct field")
expect("fn a() {\n    let f =\n        // long one\n        |x| x + 1;\n}\n", "", "after an equals")

-- 12. Anything else near a closure is still a body comment. rustfmt starts a
-- continued `||`, `|` or or-pattern line with the operator, so a leading pipe
-- is a closure only after an opener.
expect("fn a() {\n    let ok = a\n        // why\n        || b;\n}\n", "3", "boolean continuation")
expect("fn a() {\n    let b = a\n        // why\n        | c;\n}\n", "3", "bit-or continuation")
expect("fn a() {\n    match k {\n        A\n        // why\n        | B => 1,\n    }\n}\n", "4", "or-pattern continuation")
expect("fn a() {\n    match k {\n        A => 1,\n        // why\n        B | C => 2,\n    }\n}\n", "4", "or-pattern arm")
expect("fn a() {\n    // not a closure\n    let x = a | b;\n}\n", "2", "bit-or")
expect("fn a() {\n    // not a closure\n    if a || b {}\n}\n", "2", "logical or")
expect("fn a() {\n    // not a closure\n    match x {\n        A | B => 1,\n    }\n}\n", "2", "or-pattern")
expect("fn a() {\n    let f = |x| x; // trailing\n}\n", "2", "trailing on a closure line")
expect("fn a() {\n    let f = |x| {\n        // inside\n        x\n    };\n}\n", "3", "inside a closure")
expect("fn a() {\n    // gap\n\n    let f = |x| x;\n}\n", "2", "blank line between")

-- 13. Braces that never close are an error, not a pass.
local found, err = finder.find("fn a() {\n")
assert(found == nil and err, "unbalanced braces should return nil and a reason")

-- The hook itself, in a scratch repo.
local function sh(cmd)
    local p = assert(io.popen(cmd .. " 2>&1", "r"))
    local out = p:read("a")
    local _, _, code = p:close()
    return out, code
end

local dir = os.tmpname()
os.remove(dir)
assert(os.execute("mkdir -p '" .. dir .. "/.githooks'"))
assert(os.execute("cp '" .. root .. "/no_body_comments.lua' '" .. dir .. "/.githooks/'"))
sh("cd '" .. dir .. "' && git init -q && git config user.email t@t && git config user.name t")

local function write(name, text)
    assert(os.execute("mkdir -p '" .. dir .. "/" .. (name:match("^(.*)/[^/]*$") or ".") .. "'"))
    local f = assert(io.open(dir .. "/" .. name, "w"))
    f:write(text)
    f:close()
end

local function stage(name, text)
    write(name, text)
    sh("cd '" .. dir .. "' && git add -f -- '" .. name .. "'")
end

local function unstage_all()
    sh("cd '" .. dir .. "' && git rm -rq --cached -f . 2>/dev/null; true")
end

local function check(flag)
    return sh("cd '" .. dir .. "' && lua .githooks/no_body_comments.lua " .. (flag or ""))
end

local BAD = "fn a() {\n    // why\n    let x = 1;\n}\n"
local GOOD = "// why\nfn a() {\n    let x = 1;\n}\n"

-- A body comment in a staged file of each of the three crates is refused,
-- with the file and line named.
for _, path in ipairs({ "engine/src/a.rs", "models/src/a.rs", "particle-core/src/a.rs" }) do
    stage(path, BAD)
    local out, code = check()
    assert(code == 1, path .. " should refuse, got " .. tostring(code))
    assert(out:find(path .. ":2:", 1, true), path .. " should be named with its line, got: " .. out)
    unstage_all()
end

-- Other crates, and files that are not Rust, are none of its business.
stage("strings/src/a.rs", BAD)
stage("models/notes.md", "// not rust\n")
local out, code = check()
assert(code == 0, "other paths should pass, got " .. tostring(code) .. ": " .. out)
unstage_all()

-- A clean staged file passes.
stage("models/src/a.rs", GOOD)
out, code = check()
assert(code == 0, "a clean file should pass, got " .. tostring(code) .. ": " .. out)
unstage_all()

-- It reads what is staged, not the working copy.
stage("models/src/a.rs", GOOD)
write("models/src/a.rs", BAD)
out, code = check()
assert(code == 0, "a dirty working copy over a clean index should pass: " .. out)
unstage_all()
stage("models/src/a.rs", BAD)
write("models/src/a.rs", GOOD)
out, code = check()
assert(code == 1, "a clean working copy over a dirty index should refuse")
unstage_all()

-- A file it cannot read is refused, not waved through.
stage("models/src/a.rs", "fn a() {\n")
out, code = check()
assert(code == 1 and out:find("models/src/a.rs", 1, true), "an unbalanced file should refuse: " .. out)
unstage_all()

-- `--tree` checks every tracked file, staged or not.
stage("models/src/a.rs", GOOD)
sh("cd '" .. dir .. "' && git commit -qm one")
write("models/src/a.rs", BAD)
sh("cd '" .. dir .. "' && git add models/src/a.rs && git commit -qm two")
out, code = check("--tree")
assert(code == 1 and out:find("models/src/a.rs:2:", 1, true), "--tree should find it: " .. out)
out, code = check()
assert(code == 0, "nothing staged should pass: " .. out)

os.execute("rm -rf '" .. dir .. "'")
print("no_body_comments_test: ok")
