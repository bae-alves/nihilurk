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

-- 8. Per-page data for finish.lua: html path, url, and a description from the
--    page's first prose paragraph (links, backticks and emphasis stripped, the
--    indented header block, tables and fences skipped, long ones cut).
fixture()
write("docs/how-to/long.md", "Long\n====\n\n" .. string.rep("word ", 60) .. "\n")
write("docs/tutorial/d.md", "Tutorial D\n==========\n\n    Audience   Someone.\n\n```\ncode\n```\n\n| a | b |\n|---|---|\n\nA *lesson* with [a link](x.md) and `code`.\nSecond line.\n\nNext paragraph.\n")
assert(select(2, run()) == 0)
local rows = {}
for line in read("pages.tsv"):gmatch("[^\n]+") do
  local html, url, desc = line:match("^([^\t]*)\t([^\t]*)\t([^\t]*)$")
  assert(html, "pages.tsv row has three fields: " .. line)
  rows[html] = { url = url, desc = desc }
end
assert(rows["index.html"].url == "https://example.test/nihilurk/", "README is index.html")
assert(rows["docs/index.html"].url == "https://example.test/nihilurk/docs/", "docs README is docs/index.html")
assert(rows["MANUAL.html"].desc == "How to play.", "first paragraph is the description")
assert(rows["docs/tutorial/d.html"].desc == "A lesson with a link and code. Second line.", "markup stripped, header block and table skipped: " .. rows["docs/tutorial/d.html"].desc)
local long = rows["docs/how-to/long.html"].desc
assert(#long <= 160 and long:sub(-3) == "..." and not long:find("%s%.%.%.$"), "long description is cut at a word: " .. long)
assert(rows["docs/how-to/a.html"].desc == "See ../reference/b.md.", "a page's own words")

-- 9. The image is staged next to the pages, and lastmod comes from git when
--    the tree is a repository.
write("site/og-image.png", "png")
assert(select(2, run()) == 0)
assert(has(sh("ls '" .. out_dir .. "/src'"), "og-image.png"), "og image is staged")
assert(not has(read("src/sitemap.xml"), "lastmod"), "no git, no lastmod")
sh("cd '" .. dir .. "' && git init -q && git add -A && git -c user.name=t -c user.email=t@t commit -qm x")
assert(select(2, run()) == 0)
assert(read("src/sitemap.xml"):find("<loc>https://example.test/nihilurk/MANUAL.html</loc><lastmod>%d%d%d%d%-%d%d%-%d%d</lastmod>"), "lastmod from git")

-- 10. finish.lua gives each built page its own description, a canonical url
--     and og:url. It leaves pages that are not in the list (print, 404) alone.
local finish = build:gsub("build%.lua$", "finish.lua")
local book = out_dir .. "/book"
local function page(desc)
  return '<head>\n<meta property="og:description" content="' .. desc .. '">\n<meta name="description" content="' .. desc .. '">\n</head>\n'
end
sh("mkdir -p '" .. book .. "/docs/how-to'")
local function put(path, text)
  sh("mkdir -p \x27" .. book .. "/" .. (path:match("^(.*)/[^/]*$") or ".") .. "\x27")
  local f = assert(io.open(book .. "/" .. path, "w")); f:write(text); f:close()
end
local function got(path) local f = assert(io.open(book .. "/" .. path)); local s = f:read("a"); f:close(); return s end
sh("mkdir -p \x27" .. book .. "/docs\x27")
for html in pairs(rows) do put(html, page("Book &amp; description.")) end
put("print.html", page("Book &amp; description."))
o, code = sh("lua '" .. finish .. "' '" .. out_dir .. "'")
assert(code == 0, "finish should succeed: " .. o)
local m = got("MANUAL.html")
assert(has(m, '<meta name="description" content="How to play.">') and has(m, '<meta property="og:description" content="How to play.">'), "description replaced everywhere")
assert(has(m, '<link rel="canonical" href="https://example.test/nihilurk/MANUAL.html">') and has(m, '<meta property="og:url" content="https://example.test/nihilurk/MANUAL.html">'), "canonical and og:url")
assert(has(got("docs/how-to/a.html"), "content=\"See ../reference/b.md.\""), "nested page gets its own")
assert(has(got("index.html"), "content=\"Book &amp; description.\"") and has(got("index.html"), 'href="https://example.test/nihilurk/"'), "the landing page keeps the book description")
assert(got("print.html") == page("Book &amp; description."), "unlisted pages are untouched")

sh("rm -rf '" .. dir .. "'")
print("ok  site/build.lua, site/finish.lua")
