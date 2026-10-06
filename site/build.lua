-- site/build.lua -- stages the book mdBook builds.
--
--   lua site/build.lua [root] [out]
--
-- root defaults to the repository, out to <root>/target/site-book. It writes
--
--   out/book.toml, out/theme/   copied from site/, "@SITE_URL@" filled in
--   out/src/                    README.md, MANUAL.md and docs/, with a backticked
--                               path to another page turned into a link
--   out/src/SUMMARY.md          the table of contents mdBook needs
--   out/src/sitemap.xml, robots.txt, llms.txt
--
-- then `mdbook build <out>` makes the site. The one site URL lives in
-- site/book.toml (`site-url`). The recipe is docs/how-to/publish-the-site.md.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local root = arg[1] or (here .. "/..")
local out = arg[2] or (root .. "/target/site-book")
local src = out .. "/src"

local function die(msg)
  io.stderr:write("site/build.lua: ", msg, "\n")
  os.exit(1)
end

local function sh(cmd)
  local p = assert(io.popen(cmd, "r"))
  local s = p:read("a")
  p:close()
  return s
end

local function q(s) return "'" .. s:gsub("'", "'\\''") .. "'" end

local function read(path)
  local f = io.open(path)
  if not f then die("cannot read " .. path) end
  local s = f:read("a")
  f:close()
  return s
end

local function write(path, text)
  assert(os.execute("mkdir -p " .. q(path:match("^(.*)/[^/]*$") or ".")))
  local f = assert(io.open(path, "w"))
  f:write(text)
  f:close()
end

local function lines(s)
  local t = {}
  for line in (s .. "\n"):gmatch("(.-)\n") do t[#t + 1] = line end
  return t
end

local book_toml = read(root .. "/site/book.toml")
local site_url = book_toml:match('site%-url%s*=%s*"([^"]+)"') or die("site/book.toml has no site-url")
if site_url:sub(-1) ~= "/" then site_url = site_url .. "/" end

local function fill(s) return (s:gsub("@SITE_URL@", site_url)) end

-- Pages: README.md, MANUAL.md, then every .md under docs/.
local pages = { "README.md", "MANUAL.md" }
for path in sh("cd " .. q(root) .. " && find docs -name '*.md' | LC_ALL=C sort"):gmatch("[^\n]+") do
  pages[#pages + 1] = path
end

local staged = {}
for _, p in ipairs(pages) do staged[p] = true end

-- A page's title is its first heading, setext or ATX.
local function title_of(path, text)
  local l = lines(text)
  for i, line in ipairs(l) do
    local atx = line:match("^#%s+(.-)%s*$")
    if atx then return atx end
    if line:match("%S") and l[i + 1] and l[i + 1]:match("^=+%s*$") then return line end
    if line:match("%S") and l[i + 1] and l[i + 1]:match("^%-%-+%s*$") then return line end
  end
  die(path .. " has no title (a first heading)")
end

-- `a/../b.md` -> `b.md`; nil if it climbs out of the tree.
local function normalize(path)
  local parts = {}
  for seg in path:gmatch("[^/]+") do
    if seg == ".." then
      if #parts == 0 then return nil end
      parts[#parts] = nil
    elseif seg ~= "." then
      parts[#parts + 1] = seg
    end
  end
  return table.concat(parts, "/")
end

local function dir_of(path) return path:match("^(.*)/[^/]*$") or "" end

-- Link a backticked `x.md` that names a staged page, outside code blocks. The
-- path is tried relative to the page, then relative to the repository.
local function link_paths(page, text)
  local base = dir_of(page)
  local fenced = false
  local res = {}
  for _, line in ipairs(lines(text)) do
    if line:match("^%s*```") then
      fenced = not fenced
    elseif not fenced and not line:match("^    ") then
      line = line:gsub("`([^`%s]+%.md)`", function(p)
        if p:match("^/") or p:match("^%a+:") then return nil end
        if staged[normalize((base ~= "" and base .. "/" or "") .. p) or ""] then
          return "[`" .. p .. "`](" .. p .. ")"
        end
        local up = normalize(p)
        if up and staged[up] then
          return "[`" .. p .. "`](" .. string.rep("../", select(2, base:gsub("[^/]+", ""))) .. up .. ")"
        end
      end)
    end
    res[#res + 1] = line
  end
  return table.concat(res, "\n")
end

assert(os.execute("rm -rf " .. q(out)))
local titles = {}
for _, p in ipairs(pages) do
  local text = read(root .. "/" .. p)
  titles[p] = title_of(p, text)
  write(src .. "/" .. p, link_paths(p, text))
end

-- SUMMARY.md
local sections = {
  { "Tutorials", "docs/tutorial/" }, { "How-to guides", "docs/how-to/" },
  { "Reference", "docs/reference/" }, { "Explanation", "docs/explanation/" },
}
-- mdBook's page title is "<label> - <book title>", so the two pages that
-- share the book's name get labels that say what they are.
local s = { "# Summary", "", "[A Rogue-like for the terminal](README.md)", "[Instruction manual: how to play](MANUAL.md)", "" }
for _, sec in ipairs(sections) do
  s[#s + 1] = "# " .. sec[1]
  s[#s + 1] = ""
  for _, p in ipairs(pages) do
    if p:sub(1, #sec[2]) == sec[2] then s[#s + 1] = "- [" .. titles[p] .. "](" .. p .. ")" end
  end
  s[#s + 1] = ""
end
s[#s + 1] = "[" .. titles["docs/README.md"] .. "](docs/README.md)"
write(src .. "/SUMMARY.md", table.concat(s, "\n") .. "\n")

-- Crawler files. A README is its directory's page.
local function url_of(p)
  if p == "README.md" then return site_url end
  if p:match("README%.md$") then return site_url .. dir_of(p) .. "/" end
  return site_url .. p:gsub("%.md$", ".html")
end
local sm = { '<?xml version="1.0" encoding="UTF-8"?>', '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' }
for _, p in ipairs(pages) do sm[#sm + 1] = "  <url><loc>" .. url_of(p) .. "</loc></url>" end
sm[#sm + 1] = "</urlset>"
write(src .. "/sitemap.xml", table.concat(sm, "\n") .. "\n")
write(src .. "/robots.txt", "User-agent: *\nAllow: /\n\nSitemap: " .. site_url .. "sitemap.xml\n")
write(src .. "/llms.txt", fill(read(root .. "/site/llms.txt")))

-- mdBook inputs.
write(out .. "/book.toml", book_toml)
write(out .. "/theme/head.hbs", fill(read(root .. "/site/theme/head.hbs")))
