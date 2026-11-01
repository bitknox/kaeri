-- The subtitle style panel, on Alt+S: size, raise, background and styled subtitles, drawn as a
-- card like the subtitle-sync guide. Up and Down pick a row, Left and Right change it, Esc closes.
--
-- The changes are mpv's own subtitle properties, so they show at once, and the host keeps each
-- one as a setting, the same as Settings, Subtitles.

local app = require("app")
local card = require("card")

local M = {}

-- Each row is a setting under [subtitles] in the config, in its own words, so what is chosen
-- here is kept for every session: the host is told with a "sub-style" event.
local BACKGROUNDS = {
    { "None", "none", "outline-and-shadow" },
    { "Each line", "each-line", "opaque-box" },
    { "All lines", "all-lines", "opaque-box" },
}

local STYLED = {
    { "Keep their look", "keep", "no" },
    { "Restyle", "restyle", "force" },
    { "Remove all styling", "strip", "strip" },
}

local util = require("util")

local open = false
local row = 1
-- What the panel last set, by setting name, for the rows mpv cannot read back exactly.
local chosen = { background = 1, styled = 1 }

-- Pure: `value` moved by `step`, held within `low`..`high`.
function M.nudge(value, step, low, high)
    return math.max(low, math.min(high, value + step))
end

-- Pure: the index in a list of choices after one step either way, stopping at the ends.
function M.choose(index, step, count)
    return math.max(1, math.min(count, index + step))
end

local function number(name, fallback)
    local value = mp.get_property_native(name)
    return type(value) == "number" and value or fallback
end

local function size_now()
    return math.floor(number("sub-scale", 1) * 100 + 0.5)
end

local function raise_now()
    return math.floor(100 - number("sub-pos", 100) + 0.5)
end

local function rows_now()
    return {
        { "Size", string.format("%d%%", size_now()) },
        { "Raise", string.format("%d%%", raise_now()) },
        { "Background", BACKGROUNDS[chosen.background][1] },
        { "Styled subtitles", STYLED[chosen.styled][1] },
    }
end

local function change(step)
    if row == 1 then
        local size = M.nudge(size_now(), step * 10, 50, 200)
        mp.set_property_native("sub-scale", size / 100)
        util.emit("sub-style", { setting = "size", value = size })
    elseif row == 2 then
        -- Raising lowers sub-pos: 100 is the bottom edge.
        local raised = M.nudge(raise_now(), step * 2, 0, 20)
        mp.set_property_native("sub-pos", 100 - raised)
        util.emit("sub-style", { setting = "raise", value = raised })
    elseif row == 3 then
        chosen.background = M.choose(chosen.background, step, #BACKGROUNDS)
        local entry = BACKGROUNDS[chosen.background]
        mp.set_property_native("sub-border-style", entry[3])
        util.emit("sub-style", { setting = "background", value = entry[2] })
    else
        chosen.styled = M.choose(chosen.styled, step, #STYLED)
        local entry = STYLED[chosen.styled]
        mp.set_property_native("sub-ass-override", entry[3])
        util.emit("sub-style", { setting = "styled", value = entry[2] })
    end
    app.changed()
end

function M.toggle()
    open = not open
    row = 1
    app.changed()
end

function M.close()
    if open then
        open = false
        app.changed()
    end
end

function M.is_open()
    return open
end

M.element = card.element("top-center", function()
    local rows = {}
    for index, entry in ipairs(rows_now()) do
        local current = index == row
        rows[#rows + 1] = {
            left = { { parts = { { entry[1], current and "primary" or "quiet" } } } },
            right = { { parts = { { (current and "‹ " or "  ") .. entry[2] .. (current and " ›" or ""), current and "body" or "quiet" } } } },
        }
    end
    rows[#rows + 1] = {
        left = { { parts = { { "Up and Down pick, Left and Right change", "hint" } } } },
        right = { { id = "style-close", fn = M.close, parts = { { "Close", "quiet" }, { "Esc", "hint" } } } },
    }
    return { obi = true, rows = rows }
end)
M.element.id = "substyle"

function M.element.wants()
    return open
end

function M.element.keys()
    local function move(step)
        return function()
            row = M.choose(row, step, 4)
            app.changed()
        end
    end
    return {
        UP = move(-1),
        DOWN = move(1),
        LEFT = function() change(-1) end,
        RIGHT = function() change(1) end,
        ESC = M.close,
    }
end

return M
