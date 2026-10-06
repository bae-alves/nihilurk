-- lua site/build_test.lua
--
-- Runs site/build.lua on a throwaway tree shaped like this repository: a
-- README, a manual, a docs/ with one page per section, and the site/ inputs.
-- No mdBook needed: the script only stages files, and mdBook reads them later.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local build = io.popen("cd '" .. here .. "' && pwd"):read("l") .. "/build.lua"

local function sh(cmd)
  local p = assert(io.popen(cmd .. " 2>&1", "r"))
  local out = p:read("a")
  local _, _, code = p:close()
  return out, code
end

local dir = os.tmpname()
os.remove(dir)
local out_dir = dir .. "/target/site-book"

local function write(path, text)
  assert(os.execute("mkdir -p '" .. dir .. "/" .. (path:match("^(.*)/[^/]*$") or ".") .. "'"))
  local f = assert(io.open(dir .. "/" .. path, "w"))
  f:write(text)
  f:close()
end
local function read(path)
  local f = assert(io.open(out_dir .. "/" .. path))
  local s = f:read("a")
  f:close()
  return s
end
local function has(s, needle) return s:find(needle, 1, true) ~= nil end

local function fixture()
  sh("rm -rf '" .. dir .. "'")
  write("README.md", "nihilurk\n========\n\nPlay with `MANUAL.md`. Build with `docs/how-to/a.md`. Not `docs/nope.md`.\n")
  write("MANUAL.md", "nihilurk: instruction manual\n============================\n\nHow to play.\n")
  write("docs/README.md", "nihilurk documentation\n======================\n\n| I want to | Read |\n|---|---|\n| A | `how-to/a.md` |\n")
  write("docs/how-to/a.md", "How to a\n========\n\nSee `../reference/b.md`.\n\n    cat `../reference/b.md`\n\n```\nsee `../reference/b.md`\n```\n")
  write("docs/reference/b.md", "# B reference\n\nText.\n")
  write("docs/explanation/c.md", "C, explained\n------------\n\nText.\n")
  write("docs/tutorial/d.md", "Tutorial D\n==========\n\nText.\n")
  write("site/book.toml", '[book]\ntitle = "t"\n\n[output.html]\nsite-url = "https://example.test/nihilurk/"\n')
  write("site/theme/head.hbs", '<meta property="og:url" content="@SITE_URL@">\n')
  write("site/llms.txt", "# nihilurk\nSite: @SITE_URL@\n")
end

local function run() return sh("lua '" .. build .. "' '" .. dir .. "' '" .. out_dir .. "'") end

-- 1. A clean build.
fixture()
local o, code = run()
assert(code == 0, "build should succeed: " .. o)

-- 2. The book's own config and theme carry the one site URL.
assert(has(read("book.toml"), 'site-url = "https://example.test/nihilurk/"'), "book.toml is copied")
local head = read("theme/head.hbs")
assert(has(head, "https://example.test/nihilurk/") and not has(head, "@SITE_URL@"), "head.hbs gets the site url")

-- 3. A backticked path becomes a link only when it names a staged page.
local readme = read("src/README.md")
assert(has(readme, "[`MANUAL.md`](MANUAL.md)"), "root-relative path links")
assert(has(readme, "[`docs/how-to/a.md`](docs/how-to/a.md)"), "nested path links")
assert(has(readme, "Not `docs/nope.md`."), "a path that names nothing stays as it was")
assert(has(read("src/docs/README.md"), "[`how-to/a.md`](how-to/a.md)"), "page-relative path links")
local a = read("src/docs/how-to/a.md")
assert(has(a, "See [`../reference/b.md`](../reference/b.md)."), "parent-relative path links")
assert(has(a, "    cat `../reference/b.md`\n"), "indented code is left alone")
assert(has(a, "```\nsee `../reference/b.md`\n```"), "fenced code is left alone")

-- 4. SUMMARY: the landing page, the manual, then docs by section, titled from
--    the page's own heading (setext or ATX).
local summary = read("src/SUMMARY.md")
local order = { "[A Rogue-like for the terminal](README.md)", "[Instruction manual: how to play](MANUAL.md)",
  "# Tutorials", "[Tutorial D](docs/tutorial/d.md)", "# How-to guides", "[How to a](docs/how-to/a.md)",
  "# Reference", "[B reference](docs/reference/b.md)", "# Explanation", "[C, explained](docs/explanation/c.md)",
  "[nihilurk documentation](docs/README.md)" }
local at = 0
for _, want in ipairs(order) do
  local s = summary:find(want, at + 1, true)
  assert(s, "SUMMARY.md should list '" .. want .. "' in order:\n" .. summary)
  at = s
end

-- 5. The crawler files.
local sitemap = read("src/sitemap.xml")
for _, loc in ipairs({ "https://example.test/nihilurk/</loc>", "https://example.test/nihilurk/MANUAL.html</loc>",
  "https://example.test/nihilurk/docs/</loc>", "https://example.test/nihilurk/docs/how-to/a.html</loc>" }) do
  assert(has(sitemap, "<loc>" .. loc), "sitemap should list " .. loc)
end
assert(not has(sitemap, "README"), "README pages are the directory url")
assert(has(read("src/robots.txt"), "Sitemap: https://example.test/nihilurk/sitemap.xml"), "robots points at the sitemap")
local llms = read("src/llms.txt")
assert(has(llms, "Site: https://example.test/nihilurk/") and not has(llms, "@SITE_URL@"), "llms.txt gets the site url")

-- 6. It refuses a page with no title, and says which.
fixture()
write("docs/how-to/untitled.md", "Just text, no heading.\n")
o, code = run()
assert(code ~= 0 and has(o, "untitled.md"), "a page with no title should fail and be named: " .. o)

-- 7. A rebuild starts clean: a page deleted from docs/ is gone from the output.
fixture()
assert(run() and has(sh("ls '" .. out_dir .. "/src/docs/how-to'"), "a.md"))
sh("rm '" .. dir .. "/docs/how-to/a.md'")
write("docs/how-to/a.md", "How to a\n========\n")
sh("rm '" .. dir .. "/docs/tutorial/d.md'")
assert(select(2, run()) == 0)
assert(not has(sh("ls '" .. out_dir .. "/src/docs/tutorial' 2>&1"), "d.md"), "stale pages are removed")

sh("rm -rf '" .. dir .. "'")
print("ok  site/build.lua")
