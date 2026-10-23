-- site/finish.lua -- gives each page mdBook built its own SEO tags.
--
--   lua site/finish.lua [out]
--
-- out defaults to target/site-book. For every page listed in out/pages.tsv
-- (written by site/build.lua) it edits out/book/<page>.html: the description
-- meta, og:description and twitter:description take the page's own
-- description, and a canonical link and og:url name its url. mdBook cannot do
-- either itself: its template has no per-page description and gives the
-- source's .md path, not the html url. Pages not in the list (print, 404) are
-- left alone.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local out = arg[1] or (here .. "/../target/site-book")

local function read(path)
  local f = io.open(path)
  if not f then io.stderr:write("site/finish.lua: cannot read ", path, "\n") os.exit(1) end
  local s = f:read("a")
  f:close()
  return s
end

local function esc(s)
  return (s:gsub("&", "&amp;"):gsub("<", "&lt;"):gsub(">", "&gt;"):gsub('"', "&quot;"))
end

-- Replace every plain occurrence of `old` in `s` with `new`.
local function replace(s, old, new)
  local res, at = {}, 1
  while true do
    local a, b = s:find(old, at, true)
    if not a then break end
    res[#res + 1] = s:sub(at, a - 1)
    res[#res + 1] = new
    at = b + 1
  end
  res[#res + 1] = s:sub(at)
  return table.concat(res)
end

for row in read(out .. "/pages.tsv"):gmatch("[^\n]+") do
  local html, url, desc = row:match("^([^\t]*)\t([^\t]*)\t([^\t]*)$")
  local path = out .. "/book/" .. html
  local text = read(path)
  -- The description as mdBook escaped it; og: and twitter: carry the same text.
  local book_desc = text:match('<meta name="description" content="(.-)">')
  if book_desc and desc ~= "" then text = replace(text, book_desc, esc(desc)) end
  local tags = '<link rel="canonical" href="' .. url .. '">\n<meta property="og:url" content="' .. url .. '">\n'
  text = text:gsub("</head>", function() return tags .. "</head>" end, 1)
  local f = assert(io.open(path, "w"))
  f:write(text)
  f:close()
end
