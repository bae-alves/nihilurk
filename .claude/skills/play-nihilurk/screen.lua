-- Rebuilds the screen a terminal would show from the bytes a program wrote.
--
--   lua screen.lua FILE [END] [ROWS COLS] [--bg]
--
-- prints the screen after the first END bytes of FILE (all of it by default),
-- on a ROWS x COLS grid (25 x 80 by default). nihilurk paints with absolute
-- cursor moves, colour codes and a few mode switches, so this reads those and
-- ignores what it does not know. No scrollback, no wrapping: whatever falls
-- off the last column or row is dropped.

local M = {}

local function utf8_len(lead)
    if lead >= 0xF0 then return 4 end
    if lead >= 0xE0 then return 3 end
    if lead >= 0xC0 then return 2 end
    return 1
end

-- crossterm's names for the sixteen ANSI colours, by index.
local NAMES = {
    [0] = "Black", "DarkRed", "DarkGreen", "DarkYellow", "DarkBlue", "DarkMagenta", "DarkCyan", "Grey",
    "DarkGrey", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White",
}

local function colour_name(index)
    return NAMES[index] or ("ansi" .. tostring(index))
end

--- What the terminal would hold after `raw`: the glyph grid and, per cell, the
--- background colour name (nil for the default one).
local function run(raw, rows, cols)
    rows, cols = rows or 25, cols or 80
    local grid, bgs = {}, {}
    local bg = nil    local function clear_row(r, from, to)
        for c = from, to do
            grid[r][c] = " "
            bgs[r][c] = nil
        end
    end
    for r = 1, rows do
        grid[r], bgs[r] = {}, {}
        clear_row(r, 1, cols)
    end

    local row, col = 1, 1
    local i, n = 1, #raw
    while i <= n do
        local b = raw:byte(i)
        if b == 0x1B then
            local _, e, params, final = raw:find("^%[([%d;?]*)(%a)", i + 1)
            if e then
                local nums = {}
                for num in params:gmatch("%d+") do nums[#nums + 1] = tonumber(num) end
                local a, c2 = nums[1], nums[2]
                if final == "H" or final == "f" then
                    row = math.min(math.max(a or 1, 1), rows)
                    col = math.max(c2 or 1, 1)
                elseif final == "A" then row = math.max(row - (a or 1), 1)
                elseif final == "B" then row = math.min(row + (a or 1), rows)
                elseif final == "C" then col = col + (a or 1)
                elseif final == "D" then col = math.max(col - (a or 1), 1)
                elseif final == "G" then col = math.max(a or 1, 1)
                elseif final == "d" then row = math.min(math.max(a or 1, 1), rows)
                elseif final == "K" then
                    local mode = a or 0
                    if mode == 0 then clear_row(row, math.min(col, cols + 1), cols)
                    elseif mode == 1 then clear_row(row, 1, math.min(col, cols))
                    else clear_row(row, 1, cols) end
                elseif final == "m" then
                    local k = 1
                    if #nums == 0 then bg = nil end
                    while k <= #nums do
                        local code = nums[k]
                        if code == 0 or code == 49 then bg = nil
                        elseif code >= 40 and code <= 47 then bg = colour_name(code - 40)
                        elseif code >= 100 and code <= 107 then bg = colour_name(code - 100 + 8)
                        elseif code == 48 or code == 38 then
                            local kind = nums[k + 1]
                            local width = kind == 5 and 2 or kind == 2 and 4 or 0
                            if code == 48 then bg = kind == 5 and colour_name(nums[k + 2]) or "rgb" end
                            k = k + width
                        end
                        k = k + 1
                    end
                elseif final == "J" then
                    local mode = a or 0
                    if mode == 2 or mode == 3 then
                        for r = 1, rows do clear_row(r, 1, cols) end
                    elseif mode == 0 then
                        clear_row(row, math.min(col, cols + 1), cols)
                        for r = row + 1, rows do clear_row(r, 1, cols) end
                    else
                        for r = 1, row - 1 do clear_row(r, 1, cols) end
                        clear_row(row, 1, math.min(col, cols))
                    end
                end
                i = e + 1
            elseif raw:sub(i + 1, i + 1) == "]" then
                local bel = raw:find("\7", i + 2, true)
                local st = raw:find("\27\\", i + 2, true)
                i = (bel and (not st or bel < st)) and bel + 1 or (st and st + 2 or n + 1)
            elseif raw:sub(i + 1, i + 1):find("[()*+]") then
                i = i + 3
            else
                i = i + 2
            end
        elseif b == 13 then
            col, i = 1, i + 1
        elseif b == 10 then
            row, i = math.min(row + 1, rows), i + 1
        elseif b == 8 then
            col, i = math.max(col - 1, 1), i + 1
        elseif b < 32 or b == 127 then
            i = i + 1
        else
            local len = utf8_len(b)
            if col <= cols then
                grid[row][col] = raw:sub(i, i + len - 1)
                bgs[row][col] = bg
            end
            col, i = col + 1, i + len
        end
    end

    return grid, bgs
end

--- The screen after `raw`, as rows joined by newlines with trailing spaces
--- trimmed.
function M.render(raw, rows, cols)
    local grid = run(raw, rows, cols)
    local out = {}
    for r = 1, #grid do
        out[r] = table.concat(grid[r]):gsub("%s+$", "")
    end
    return table.concat(out, "\n")
end

--- Every cell on a coloured background, row by row, as `"ROW COL GLYPH NAME"`
--- (1-based, crossterm's colour names). This is how a tint shows up: the
--- glyph screen alone says nothing about colour.
function M.backgrounds(raw, rows, cols)
    local grid, bgs = run(raw, rows, cols)
    local out = {}
    for r = 1, #grid do
        for c = 1, #grid[r] do
            if bgs[r][c] then out[#out + 1] = string.format("%d %d %s %s", r, c, grid[r][c], bgs[r][c]) end
        end
    end
    return out
end

if arg and arg[0] and arg[0]:match("screen%.lua$") and arg[1] then
    local words, want_bg = {}, false
    for _, a in ipairs(arg) do
        if a == "--bg" then want_bg = true else words[#words + 1] = a end
    end
    local f = assert(io.open(words[1], "rb"))
    local raw = f:read("a")
    f:close()
    local stop = tonumber(words[2])
    if stop then raw = raw:sub(1, stop) end
    if want_bg then
        print(table.concat(M.backgrounds(raw, tonumber(words[3]), tonumber(words[4])), "\n"))
    else
        print(M.render(raw, tonumber(words[3]), tonumber(words[4])))
    end
end

return M
