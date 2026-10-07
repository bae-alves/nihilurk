-- lua .claude/skills/play-nihilurk/screen_test.lua
--
-- screen.lua rebuilds what a terminal would show from the bytes the game
-- wrote. nihilurk paints with absolute cursor moves (CSI row;col H), colour
-- codes and a few mode switches, so that is what this has to get right.

local here = (arg[0]:match("^(.*)/[^/]*$")) or "."
local screen = dofile(here .. "/screen.lua")

local function show(raw, rows, cols)
    return screen.render(raw, rows or 4, cols or 10)
end

local function eq(got, want, what)
    assert(got == want, what .. ": want [" .. want:gsub("\n", "|") .. "], got [" .. got:gsub("\n", "|") .. "]")
end

-- 1. Text lands at the cursor, and a cursor move puts it at row;col (1-based).
eq(show("hi"), "hi\n\n\n", "text at home")
eq(show("\27[3;5Hhi"), "\n\n    hi\n", "absolute move")

-- 2. A later write over the same cells wins.
eq(show("\27[1;1Hhello\27[1;3HXY"), "heXYo\n\n\n", "overwrite")

-- 3. Colour codes and mode switches take no room.
eq(show("\27[?1049h\27[?25l\27[38;5;9mred\27[0m!"), "red!\n\n\n", "sgr and modes")

-- 4. A multi-byte glyph is one cell.
eq(show("a\u{B7}b"), "a\u{B7}b\n\n\n", "utf-8 glyph")
eq(show("\u{B7}\27[1;3Hz"), "\u{B7} z\n\n\n", "utf-8 glyph is one column")

-- 5. Clearing: whole screen, rest of line, relative moves.
eq(show("abc\27[2Jx"), "   x\n\n\n", "clear keeps the cursor")
eq(show("abcdef\27[1;3H\27[K"), "ab\n\n\n", "erase to end of line")
eq(show("ab\27[2Dxy\27[2B\27[3Cz"), "xy\n\n     z\n", "relative moves")

-- 6. Carriage return and line feed.
eq(show("ab\r\ncd"), "ab\ncd\n\n", "crlf")

-- 7. Anything past the last column or row is dropped, not wrapped.
eq(show("0123456789AB"), "0123456789\n\n\n", "past the last column")
eq(show("a\r\nb\r\nc\r\nd\r\ne"), "a\nb\nc\ne", "past the last row stays on it")

-- 8. Trailing spaces are trimmed, so a blank row is empty.
eq(show("ab  \27[2;1H   "), "ab\n\n\n", "trailing spaces")

-- 9. Charset and OSC escapes are skipped whole.
eq(show("\27(Bx\27]0;title\7y"), "xy\n\n\n", "charset and osc")

-- 10. Backgrounds: which cells sit on a coloured background, by crossterm's name.
local function bgs(raw)
    return table.concat(screen.backgrounds(raw, 4, 10), "|")
end

eq(bgs("plain"), "", "no background")
eq(bgs("\27[48;5;14mX\27[49mY"), "1 1 X Cyan", "256-colour background, reset by 49")
eq(bgs("\27[48;5;3mab\27[0mc"), "1 1 a DarkYellow|1 2 b DarkYellow", "reset by 0, two cells")
eq(bgs("\27[46mq\27[mr"), "1 1 q DarkCyan", "basic and bright codes, reset by an empty m")
eq(bgs("\27[106mq"), "1 1 q Cyan", "bright basic background")
eq(bgs("\27[38;5;14;48;5;4mz"), "1 1 z DarkBlue", "foreground and background in one sequence")
eq(bgs("\27[48;5;14mX\27[49m\27[1;1HY"), "", "a later write without a background replaces it")
eq(bgs("\27[48;5;14m \27[49mY"), "1 1   Cyan", "a blank cell on a background still counts")

print("screen_test: ok")
