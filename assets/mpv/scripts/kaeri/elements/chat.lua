-- Watch together chat over the video: the newest lines in a column at the top leading corner,
-- under any notice, and a reaction's moment at the bottom trailing corner, above the corner card
-- when one is up. Nothing here takes clicks or keys; the terminal's Together section is where
-- the conversation is read in full and typed.
--
-- The column holds three lines. More wait their turn behind a fold line ("2 more in chat"),
-- which the host words for every count, so nothing is pluralised here. A line holds 4 s, one
-- more second for every ten words past twelve, then fades out and the next waiting line takes
-- its place.
--
-- Like everything the companion draws, this only fades. When a line leaves, the lines under it
-- crossfade from where they were to where they now sit rather than sliding up.

local app = require("app")
local config = require("config")
local draw = require("draw")
local overlay = require("overlay")
local util = require("util")

local M = { id = "chat", region = "canvas" }

-- Base metrics, at scale 1: the board's mpv column and moment.
-- The column's leading edge and top: the notices' own edge, so a notice and the lines under it
-- read as one column. (The board draws 24; the companion's notices sit at 16.)
local LEAD = 16
local SIZE = 18
local LINE = 24
local PAD_Y = 9
local PAD_X = 16
local RADIUS = 10
local MAX_WIDTH = 520
local GAP = 6 -- between lines, and between a notice and the column
local NAME_GAP = 6
local MAX_ROWS = 3 -- a long line wraps to three rows and ends in an ellipsis
local COLUMN = 3
local WAITING = 40 -- the host words this many fold counts; older waiting lines are dropped
local TRAIL = 24 -- moment's trailing edge
local BOTTOM = 96 -- moment's bottom when no corner card is up
local PILL = 44
local PILL_LEFT = 12
local PILL_RIGHT = 18
local GLYPH = 24
local GLYPH_GAP = 8
local STROKE = 1.6
local MOMENTS = 4

-- Timings, in seconds.
local FADE_IN = 0.2
local FADE_OUT = 0.3
local CROSSFADE = 0.2
local MOMENT = 1.5

local GROUND = 0.88
local HAIR = 0.08

local shown = {} -- lines in the column, oldest first
local waiting = {} -- lines behind the fold
local more = {} -- the fold line's wording per count, from the host
local moments = {} -- reactions on screen, oldest first
local fold = { gone = nil } -- the fold line's own fade and place
local frame_due = false

local function now()
    return mp.get_time()
end

local function ease(t)
    t = math.max(0, math.min(1, t))
    return 1 - (1 - t) * (1 - t)
end

-- An item's opacity from its arrival and, once it is leaving, its departure.
local function presence(item, t)
    local a = ease((t - item.born) / FADE_IN)
    if item.leaving then
        a = math.min(a, 1 - ease((t - item.leaving) / FADE_OUT))
    end
    return a
end

local function settled(item, t)
    return t - item.born >= FADE_IN and not item.leaving
        and (not item.moved or t - item.moved >= CROSSFADE)
end

local function changed()
    app.changed()
    overlay.request()
end

---------------------------------------------------------------------------------------------
-- The column
---------------------------------------------------------------------------------------------

local function words(text)
    local n = 0
    for _ in tostring(text):gmatch("%S+") do
        n = n + 1
    end
    return n
end

local function hold(text)
    return 4 + math.max(0, words(text) - 12) / 10
end

local promote

local function remove(list, item)
    for i, other in ipairs(list) do
        if other == item then
            table.remove(list, i)
            return
        end
    end
end

local function leave(line)
    if line.leaving then
        return
    end
    line.leaving = now()
    if line.timer then
        line.timer:kill()
        line.timer = nil
    end
    mp.add_timeout(FADE_OUT, function()
        remove(shown, line)
        promote()
        changed()
    end)
    changed()
end

local function show(line)
    line.born = now()
    shown[#shown + 1] = line
    line.timer = mp.add_timeout(hold(line.text), function()
        line.timer = nil
        leave(line)
    end)
end

local function staying()
    local n = 0
    for _, line in ipairs(shown) do
        if not line.leaving then
            n = n + 1
        end
    end
    return n
end

promote = function()
    while #shown < COLUMN and #waiting > 0 do
        show(table.remove(waiting, 1))
    end
end

function M.line(who, you, text)
    who, text = util.str(who), util.str(text)
    if not who or not text then
        return
    end
    local line = { who = who, you = you == true, text = text }
    if #shown < COLUMN and #waiting == 0 then
        show(line)
    else
        waiting[#waiting + 1] = line
        while #waiting > WAITING do
            table.remove(waiting, 1)
        end
    end
    changed()
end

function M.set_more(list)
    if type(list) == "table" and #list > 0 then
        more = list
    end
end

local function fold_text()
    local n = #waiting
    if n == 0 then
        return nil
    end
    return util.str(more[math.min(n, #more)]) or tostring(n)
end

---------------------------------------------------------------------------------------------
-- Moments
---------------------------------------------------------------------------------------------

local function retire(moment)
    if moment.leaving then
        return
    end
    moment.leaving = now()
    mp.add_timeout(FADE_OUT, function()
        remove(moments, moment)
        changed()
    end)
    changed()
end

function M.react(who, glyph)
    who, glyph = util.str(who), util.str(glyph)
    if not who or not glyph then
        return
    end
    local moment = { who = who, glyph = glyph, born = now() }
    moments[#moments + 1] = moment
    local live = 0
    for i = #moments, 1, -1 do
        if not moments[i].leaving then
            live = live + 1
            if live > MOMENTS then
                retire(moments[i])
            end
        end
    end
    mp.add_timeout(MOMENT, function()
        retire(moment)
    end)
    changed()
end

-- Hide chat turned on: everything on screen fades out and nothing waits.
function M.clear()
    waiting = {}
    for _, line in ipairs(shown) do
        leave(line)
    end
    for _, moment in ipairs(moments) do
        retire(moment)
    end
    changed()
end

function M.wants()
    return #shown > 0 or #waiting > 0 or #moments > 0
end

function M.published()
    local lines = {}
    for _, line in ipairs(shown) do
        if not line.leaving then
            lines[#lines + 1] = { who = line.who, text = line.text, you = line.you }
        end
    end
    local list = {}
    for _, moment in ipairs(moments) do
        if not moment.leaving then
            list[#list + 1] = { who = moment.who, glyph = moment.glyph }
        end
    end
    return { lines = lines, more = fold_text(), moments = list }
end

function M.keys()
    return {}
end

---------------------------------------------------------------------------------------------
-- The glyphs: the same four the apps draw, from desktop/linux/src/party/glyph.rs, on its 24
-- unit grid. Strokes are traced there and back so each has no inside, and libass's border
-- draws them as round-capped lines centred on the path.
---------------------------------------------------------------------------------------------

local TAU = math.pi * 2

local function arc(cx, cy, r, a0, a1, n)
    local pts = {}
    n = n or 24
    for i = 0, n do
        local a = a0 + (a1 - a0) * i / n
        pts[#pts + 1] = { cx + r * math.cos(a), cy + r * math.sin(a) }
    end
    return pts
end

local function cubic(p0, p1, p2, p3, n, pts)
    pts = pts or { p0 }
    n = n or 10
    for i = 1, n do
        local t = i / n
        local u = 1 - t
        pts[#pts + 1] = {
            u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1],
            u * u * u * p0[2] + 3 * u * u * t * p1[2] + 3 * u * t * t * p2[2] + t * t * t * p3[2],
        }
    end
    return pts
end

local function quad(p0, via, p2)
    local c1 = { p0[1] + 2 / 3 * (via[1] - p0[1]), p0[2] + 2 / 3 * (via[2] - p0[2]) }
    local c2 = { p2[1] + 2 / 3 * (via[1] - p2[1]), p2[2] + 2 / 3 * (via[2] - p2[2]) }
    return cubic(p0, c1, c2, p2)
end

local function append(pts, more_pts)
    for _, p in ipairs(more_pts) do
        pts[#pts + 1] = p
    end
    return pts
end

local function rounded(x, y, w, h, r)
    local pts = {}
    append(pts, arc(x + w - r, y + r, r, -math.pi / 2, 0, 6))
    append(pts, arc(x + w - r, y + h - r, r, 0, math.pi / 2, 6))
    append(pts, arc(x + r, y + h - r, r, math.pi / 2, math.pi, 6))
    append(pts, arc(x + r, y + r, r, math.pi, math.pi * 1.5, 6))
    pts[#pts + 1] = pts[1]
    return pts
end

local function rotated(pts, px, py, degrees)
    local a = math.rad(degrees)
    local c, s = math.cos(a), math.sin(a)
    local out = {}
    for i, p in ipairs(pts) do
        local dx, dy = p[1] - px, p[2] - py
        out[i] = { px + dx * c - dy * s, py + dx * s + dy * c }
    end
    return out
end

-- Strokes and fills of each glyph, as point lists on the 24 unit grid.
local function shapes(name)
    local strokes, fills = {}, {}
    local function face()
        strokes[#strokes + 1] = arc(12, 12, 9, 0, TAU, 40)
    end
    local function dot(x, y, r)
        fills[#fills + 1] = arc(x, y, r, 0, TAU, 12)
    end
    if name == "laugh" then
        face()
        for _, x in ipairs({ 8, 13 }) do
            strokes[#strokes + 1] = quad({ x, 10.3 }, { x + 1.5, 8.4 }, { x + 3, 10.3 })
        end
        local mouth = { { 7.6, 13.4 }, { 16.4, 13.4 } }
        append(mouth, arc(12, 13.4, 4.4, 0, math.pi, 16))
        mouth[#mouth + 1] = { 7.6, 13.4 }
        strokes[#strokes + 1] = mouth
    elseif name == "gasp" then
        face()
        dot(9, 10, 1)
        dot(15, 10, 1)
        strokes[#strokes + 1] = arc(12, 15.3, 2.2, 0, TAU, 16)
    elseif name == "clap" then
        for _, hand in ipairs({ { 5.6, -16, 8.5 }, { 12.6, 16, 15.5 } }) do
            strokes[#strokes + 1] = rotated(rounded(hand[1], 8.6, 5.8, 12, 2.9), hand[3], 14.6, hand[2])
        end
        strokes[#strokes + 1] = { { 12, 2.6 }, { 12, 4.9 } }
        strokes[#strokes + 1] = { { 7.4, 3.8 }, { 8.6, 5.7 } }
        strokes[#strokes + 1] = { { 16.6, 3.8 }, { 15.4, 5.7 } }
    elseif name == "cry" then
        face()
        strokes[#strokes + 1] = { { 8, 10 }, { 10.5, 10 } }
        strokes[#strokes + 1] = { { 13.5, 10 }, { 16, 10 } }
        strokes[#strokes + 1] = quad({ 9, 16.6 }, { 12, 14.3 }, { 15, 16.6 })
        local tear = cubic({ 16.2, 11.7 }, { 16.2, 11.7 }, { 15, 13.5 }, { 15, 14.3 })
        append(tear, arc(16.2, 14.3, 1.2, math.pi, 0, 10))
        cubic({ 17.4, 14.3 }, { 17.4, 13.5 }, { 16.2, 11.7 }, { 16.2, 11.7 }, 10, tear)
        fills[#fills + 1] = tear
    end
    return strokes, fills
end

local function num(v)
    return (string.format("%.2f", v):gsub("%.?0+$", ""))
end

local function path(pts, x, y, k, there_and_back)
    local out = {}
    for i, p in ipairs(pts) do
        out[#out + 1] = (i == 1 and "m " or "l ") .. num(x + p[1] * k) .. " " .. num(y + p[2] * k)
    end
    if there_and_back then
        for i = #pts - 1, 1, -1 do
            out[#out + 1] = "l " .. num(x + pts[i][1] * k) .. " " .. num(y + pts[i][2] * k)
        end
    end
    return table.concat(out, " ")
end

local function draw_glyph(ctx, name, x, y, side, opacity)
    local strokes, fills = shapes(name)
    local k = side / 24
    local alpha = draw.alpha(opacity * ctx.alpha)
    if #strokes > 0 then
        local parts = {}
        for _, pts in ipairs(strokes) do
            parts[#parts + 1] = path(pts, x, y, k, true)
        end
        ctx.out[#ctx.out + 1] = string.format(
            "{\\an7\\pos(0,0)\\shad0\\blur0\\bord%s\\1a&HFF&\\3c%s\\3a%s\\p1}%s{\\p0}",
            num(STROKE / 2 * k), config.color.text, alpha, table.concat(parts, " ")
        )
    end
    if #fills > 0 then
        local parts = {}
        for _, pts in ipairs(fills) do
            parts[#parts + 1] = path(pts, x, y, k, false)
        end
        ctx.out[#ctx.out + 1] = string.format(
            "{\\an7\\pos(0,0)\\bord0\\shad0\\blur0\\1c%s\\1a%s\\p1}%s{\\p0}",
            config.color.text, alpha, table.concat(parts, " ")
        )
    end
end

---------------------------------------------------------------------------------------------
-- Drawing
---------------------------------------------------------------------------------------------

local function font(ctx, bold, color)
    local f = ctx:font(SIZE, bold)
    f.color = color
    return f
end

-- A line's rows: the name on the first, the words flowing after it, wrapped to the column's
-- width and cut with an ellipsis on the third row. Cached per scale.
local function layout(ctx, line)
    if line.laid and line.laid.s == ctx.s then
        return line.laid
    end
    local name_font = font(ctx, true, line.you and config.color.dim or config.color.text)
    local text_font = font(ctx, false, config.color.text)
    local inner = ctx:px(MAX_WIDTH - PAD_X * 2)
    local name = line.who or ""
    local name_w = name ~= "" and ctx:width(name, name_font) + ctx:px(NAME_GAP) or 0
    if name_w > inner / 2 then
        name = ctx:fit(name, name_font, inner / 2)
        name_w = ctx:width(name, name_font) + ctx:px(NAME_GAP)
    end
    local rows = { { indent = name_w, text = "" } }
    local cut = false
    local function room(row)
        return inner - row.indent
    end
    for word in tostring(line.text):gmatch("%S+") do
        local row = rows[#rows]
        local candidate = row.text == "" and word or row.text .. " " .. word
        if ctx:width(candidate, text_font) <= room(row) then
            row.text = candidate
        else
            if row.text ~= "" then
                if #rows == MAX_ROWS then
                    cut = true
                    break
                end
                row = { indent = 0, text = "" }
                rows[#rows + 1] = row
            end
            -- A word longer than a whole row breaks where it must.
            while ctx:width(word, text_font) > room(row) do
                local head = ctx:fit(word, text_font, room(row)):gsub("…$", "")
                if head == "" then
                    break
                end
                row.text = head
                word = word:sub(#head + 1)
                if #rows == MAX_ROWS then
                    cut = true
                    break
                end
                row = { indent = 0, text = "" }
                rows[#rows + 1] = row
            end
            if cut then
                break
            end
            row.text = word
        end
    end
    if cut then
        local row = rows[#rows]
        row.text = ctx:fit(row.text .. " …", text_font, room(row))
        if not row.text:find("…$") then
            row.text = row.text .. "…"
        end
    end
    local w = 0
    for _, row in ipairs(rows) do
        w = math.max(w, row.indent + ctx:width(row.text, text_font))
    end
    line.laid = {
        s = ctx.s,
        name = name,
        name_font = name_font,
        text_font = text_font,
        rows = rows,
        w = w + ctx:px(PAD_X * 2),
        h = ctx:px(PAD_Y * 2 + LINE * #rows),
    }
    return line.laid
end

local function panel(ctx, x0, y0, x1, y1, opacity)
    local a = opacity * ctx.alpha
    local r = ctx:px(RADIUS)
    draw.rect(ctx.out, x0, y0, x1, y1, config.color.ground, GROUND * a, r)
    draw.ring(ctx.out, x0, y0, x1, y1, config.color.text, HAIR * a, r, math.max(1, draw.round(ctx:px(1))))
end

-- Where an item is drawn, and with what share of its opacity: twice while it crossfades from
-- its old place to its new one.
local function places(item, y, t)
    if item.y and math.abs(item.y - y) > 0.5 then
        item.from_y = item.drawn_y or item.y
        item.moved = t
    end
    item.y = y
    if item.moved and t - item.moved < CROSSFADE and item.from_y then
        local k = ease((t - item.moved) / CROSSFADE)
        item.drawn_y = item.from_y + (y - item.from_y) * k
        return { { item.from_y, 1 - k }, { y, k } }
    end
    item.drawn_y = y
    return { { y, 1 } }
end

local function draw_line(ctx, line, x, y, opacity)
    local laid = layout(ctx, line)
    panel(ctx, x, y, x + laid.w, y + laid.h, opacity)
    local left = x + ctx:px(PAD_X)
    local cy = y + ctx:px(PAD_Y + LINE / 2)
    for i, row in ipairs(laid.rows) do
        if i == 1 and laid.name ~= "" then
            local f = laid.name_font
            ctx:text(left, cy, 4, laid.name, { size = f.size, bold = true, color = f.color, opacity = opacity })
        end
        local f = laid.text_font
        ctx:text(left + row.indent, cy, 4, row.text, { size = f.size, color = f.color, opacity = opacity })
        cy = cy + ctx:px(LINE)
    end
end

local function draw_fold(ctx, text, x, y, opacity)
    local f = font(ctx, false, config.color.dim)
    local w = ctx:width(text, f) + ctx:px(PAD_X * 2)
    local h = ctx:px(PAD_Y * 2 + LINE)
    panel(ctx, x, y, x + w, y + h, opacity)
    ctx:text(x + ctx:px(PAD_X), y + h / 2, 4, text, { size = f.size, color = f.color, opacity = opacity })
    return h
end

local function draw_moment(ctx, moment, right, bottom, opacity)
    local f = font(ctx, true, config.color.text)
    local h = ctx:px(PILL)
    local w = ctx:px(PILL_LEFT + GLYPH + GLYPH_GAP + PILL_RIGHT) + ctx:width(moment.who, f)
    local x, y = right - w, bottom - h
    panel(ctx, x, y, right, bottom, opacity)
    local g = ctx:px(GLYPH)
    draw_glyph(ctx, moment.glyph, x + ctx:px(PILL_LEFT), y + (h - g) / 2, g, opacity)
    ctx:text(x + ctx:px(PILL_LEFT + GLYPH + GLYPH_GAP), y + h / 2, 4, moment.who,
        { size = f.size, bold = true, color = f.color, opacity = opacity })
    return h
end

local function want_frame()
    if frame_due then
        return
    end
    frame_due = true
    mp.add_timeout(1 / 60, function()
        frame_due = false
        overlay.request()
    end)
end

function M.draw(ctx, _, _, w, h)
    local t = now()
    local busy = false
    local s = ctx.s
    local insets = ctx.insets or { top = 0, bottom = 0, right = 0 }

    -- The column, under a notice when one is up.
    local x = ctx:px(LEAD)
    local y = insets.top + ctx:px(LEAD)
    local notice = ctx.placed and ctx.placed["top-left"]
    if notice and notice.alpha > 0 then
        y = math.max(y, notice.y + notice.h + ctx:px(GAP))
    end
    for _, line in ipairs(shown) do
        local opacity = presence(line, t)
        for _, at in ipairs(places(line, y, t)) do
            draw_line(ctx, line, x, at[1], opacity * at[2])
        end
        busy = busy or not settled(line, t)
        y = y + layout(ctx, line).h + ctx:px(GAP)
    end
    local text = fold_text()
    if text then
        if not fold.born or fold.leaving then
            fold.born, fold.leaving = t, nil
        end
        fold.text = text
    elseif fold.born and not fold.leaving then
        fold.leaving = t
    end
    if fold.born then
        local opacity = presence(fold, t)
        if opacity > 0 or not fold.leaving then
            for _, at in ipairs(places(fold, y, t)) do
                draw_fold(ctx, fold.text, x, at[1], opacity * at[2])
            end
        end
        busy = busy or not settled(fold, t) and opacity > 0
        if fold.leaving and t - fold.leaving >= FADE_OUT then
            fold.born, fold.leaving, fold.y, fold.moved = nil, nil, nil, nil
        end
    end

    -- Moments, newest at the bottom, stacked above the corner card when one is up.
    local right = w - math.max(ctx:px(TRAIL), insets.align_right or insets.right or 0)
    local bottom = h - math.max(ctx:px(BOTTOM), (insets.bottom or 0) + 12 * s)
    local card = ctx.placed and ctx.placed.corner
    if card and card.alpha > 0 then
        bottom = math.min(bottom, card.y - ctx:px(GAP))
    end
    for i = #moments, 1, -1 do
        local moment = moments[i]
        local opacity = presence(moment, t)
        for _, at in ipairs(places(moment, bottom, t)) do
            draw_moment(ctx, moment, right, at[1], opacity * at[2])
        end
        busy = busy or not settled(moment, t)
        bottom = bottom - ctx:px(PILL + GAP)
    end

    if busy then
        want_frame()
    end
end

return M
