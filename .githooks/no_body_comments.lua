-- Refuses a commit that stages a comment inside a function body in engine,
-- models or particle-core. Called by .githooks/pre-commit.
--
-- A comment goes above the item it describes: a doc comment, a note over the
-- signature, a file header, or the note over a catalog table that says how to
-- add a row. Inside a body, name things well or split the function. A comment
-- on its own line directly above a closure is allowed, because it describes
-- the closure the way a signature comment describes a fn.
--
-- `--tree` checks every tracked file instead of the staged ones.
--
-- Reads the staged blob, not the working copy: that is what would be
-- committed. A file whose braces do not balance is refused too, because a
-- check that cannot read a file has not passed it.

local MESSAGE = "No comments inside function bodies in engine, models or particle-core.\n"
    .. "Put the note above the function, or above a closure; or make the code say it."
local CRATES = { "engine", "models", "particle-core" }

-- rustfmt wraps a closure argument onto its own line (`|x| x + 1,`, `|| f(),`),
-- and breaks `||` chains, bit-or and or-patterns the same way: a line that
-- starts with the operator. So a leading pipe is a closure only when the line
-- above the comment opens or separates something: `(`, `[`, `,`, `=`, `:`, `=>`.
local function opens_closure(line, before)
    if line:find("move%s*|") or line:find("[%(%[,=:]%s*|") or line:find("=>%s*|") then
        return true
    end
    return before ~= nil
        and line:find("^%s*|") ~= nil
        and (before:find("[%(%[,=:]%s*$") ~= nil or before:find("=>%s*$") ~= nil)
end

local M = {}

--- Comments inside fn bodies of Rust `text`, as `{ line, a, b, text }` with
--- `a`..`b` the byte span, or `nil` and a reason when the braces do not balance.
function M.find(text)
    local stack, fn_depth, pending, paren = {}, 0, false, 0
    local comments = {}
    local n, i = #text, 1

    local function note(a, b)
        if fn_depth > 0 then comments[#comments + 1] = { a = a, b = b, text = text:sub(a, b) } end
    end

    while i <= n do
        local c = text:sub(i, i)
        local two = text:sub(i, i + 1)
        local s, e, hashes = text:find('^b?r(#*)"', i)
        if two == "//" then
            local j = text:find("\n", i, true) or n + 1
            note(i, j - 1)
            i = j
        elseif two == "/*" then
            local depth, j = 1, i + 2
            while j <= n and depth > 0 do
                local pair = text:sub(j, j + 1)
                if pair == "/*" then depth, j = depth + 1, j + 2
                elseif pair == "*/" then depth, j = depth - 1, j + 2
                else j = j + 1 end
            end
            note(i, j - 1)
            i = j
        elseif s then
            local close = '"' .. hashes
            local j = text:find(close, e + 1, true)
            if not j then return nil, "unterminated raw string" end
            i = j + #close
        elseif c == "b" and text:sub(i + 1, i + 1) == '"' then
            i = i + 1
        elseif c == '"' then
            local j = i + 1
            while j <= n and text:sub(j, j) ~= '"' do
                j = j + (text:sub(j, j) == "\\" and 2 or 1)
            end
            i = j + 1
        elseif c == "'" then
            local _, ue = text:find("^'[\xC2-\xF4][\x80-\xBF]*'", i)
            if text:sub(i + 1, i + 1) == "\\" then
                local j = text:find("'", i + 3, true)
                if not j then return nil, "unterminated char literal" end
                i = j + 1
            elseif text:sub(i + 2, i + 2) == "'" then
                i = i + 3
            elseif ue then
                i = ue + 1
            else
                i = i + 1
            end
        elseif c:find("[%a_]") then
            local _, we = text:find("^[%w_]+", i)
            if text:sub(i, we) == "fn" and text:find("^%s+[%a_]", we + 1) then
                pending, paren = true, 0
            end
            i = we + 1
        elseif c:find("%d") then
            local _, we = text:find("^[%w_]+", i)
            i = we + 1
        elseif c == "(" or c == "[" then
            if pending then paren = paren + 1 end
            i = i + 1
        elseif c == ")" or c == "]" then
            if pending then paren = paren - 1 end
            i = i + 1
        elseif c == ";" then
            if pending and paren == 0 then pending = false end
            i = i + 1
        elseif c == "{" then
            if pending and paren == 0 then
                stack[#stack + 1] = "fn"
                fn_depth, pending = fn_depth + 1, false
            else
                stack[#stack + 1] = "other"
            end
            i = i + 1
        elseif c == "}" then
            local kind = table.remove(stack)
            if not kind then return nil, "unbalanced }" end
            if kind == "fn" then fn_depth = fn_depth - 1 end
            i = i + 1
        else
            i = i + 1
        end
    end
    if #stack > 0 then return nil, "unbalanced {" end

    local lines, starts = {}, {}
    for pos, line in text:gmatch("()([^\n]*)") do
        lines[#lines + 1], starts[#starts + 1] = line, pos
    end
    local function line_at(pos)
        local lo, hi = 1, #starts
        while lo < hi do
            local mid = (lo + hi + 1) // 2
            if starts[mid] <= pos then lo = mid else hi = mid - 1 end
        end
        return lo
    end

    local found = {}
    for _, cm in ipairs(comments) do
        local line = line_at(cm.a)
        local own = text:sub(starts[line], cm.a - 1):find("^%s*$") ~= nil
        local allowed = false
        if own then
            local k = line_at(cm.b) + 1
            while lines[k] and lines[k]:find("^%s*//") do k = k + 1 end
            local j = line - 1
            while lines[j] and (lines[j]:find("^%s*//") or lines[j]:find("^%s*$")) do j = j - 1 end
            allowed = lines[k] ~= nil and opens_closure(lines[k], lines[j])
        end
        if not allowed then
            cm.line = line
            found[#found + 1] = cm
        end
    end
    return found
end

if arg and arg[0] and arg[0]:match("no_body_comments%.lua$") then
    local function shell_quote(s) return "'" .. s:gsub("'", "'\\''") .. "'" end

    local list = arg[1] == "--tree" and "git ls-files -z"
        or "git diff --cached --name-only --diff-filter=ACM -z"
    local p = io.popen(list)
    local names = p:read("a")
    p:close()

    local report = {}
    for name in names:gmatch("[^\0]+") do
        local in_scope = false
        for _, crate in ipairs(CRATES) do
            if name:sub(1, #crate + 1) == crate .. "/" and name:find("%.rs$") then in_scope = true end
        end
        if in_scope then
            local blob = io.popen("git show :" .. shell_quote(name) .. " 2>/dev/null")
            local text = blob:read("a")
            blob:close()
            local found, err = M.find(text)
            if not found then
                report[#report + 1] = "  " .. name .. ": can't read it (" .. err .. ")"
            else
                for _, cm in ipairs(found) do
                    report[#report + 1] = "  " .. name .. ":" .. cm.line .. ": " .. cm.text:match("^[^\n]*")
                end
            end
        end
    end

    if #report > 0 then
        io.stderr:write(MESSAGE, "\n", table.concat(report, "\n"), "\n")
        os.exit(1)
    end
end

return M
