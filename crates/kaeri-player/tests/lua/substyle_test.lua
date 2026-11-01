-- Run by tests/companion.rs inside mpv: the style panel's pure steps stay in range.
local options = { dir = "" }
require("mp.options").read_options(options, "substyle_test")
package.path = options.dir .. "/?.lua;" .. package.path
local substyle = require("substyle")

local cases = {
    { substyle.nudge(100, 10, 50, 200), 110 },
    { substyle.nudge(200, 10, 50, 200), 200 },
    { substyle.nudge(50, -10, 50, 200), 50 },
    { substyle.choose(1, -1, 3), 1 },
    { substyle.choose(3, 1, 3), 3 },
    { substyle.choose(2, 1, 3), 3 },
}
local failed = 0
for i, case in ipairs(cases) do
    if math.abs(case[1] - case[2]) > 1e-9 then
        mp.msg.error(string.format("case %d: wanted %s, got %s", i, tostring(case[2]), tostring(case[1])))
        failed = failed + 1
    end
end
mp.commandv("quit", failed == 0 and "0" or "1")
