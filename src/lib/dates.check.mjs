import assert from "node:assert/strict";
import { matchPreset, monthGrid, parseTime, presetRange, shiftRange, stepTime } from "./dates.ts";

const today = "2026-10-16";
assert.deepEqual(presetRange("week", today), ["2026-10-12", "2026-10-18"]);
assert.deepEqual(presetRange("lastWeek", today), ["2026-10-05", "2026-10-11"]);
assert.deepEqual(presetRange("lastMonth", "2026-03-31"), ["2026-02-01", "2026-02-28"]);
assert.deepEqual(presetRange("last30", today), ["2026-09-17", "2026-10-16"]);
assert.equal(matchPreset("2026-10-01", "2026-10-31", today), "month");
assert.equal(matchPreset("2026-10-02", "2026-10-31", today), "custom");

assert.deepEqual(shiftRange("2026-10-01", "2026-10-31", -1), ["2026-09-01", "2026-09-30"]);
assert.deepEqual(shiftRange("2026-01-01", "2026-03-31", 1), ["2026-04-01", "2026-06-30"]);
assert.deepEqual(shiftRange("2026-01-01", "2026-12-31", 1), ["2027-01-01", "2027-12-31"]);
assert.deepEqual(shiftRange("2026-10-12", "2026-10-18", 1), ["2026-10-19", "2026-10-25"]);
assert.deepEqual(shiftRange("2026-10-16", "2026-10-16", -1), ["2026-10-15", "2026-10-15"]);

const grid = monthGrid("2026-10-16");
assert.equal(grid.length, 42);
assert.equal(grid[0], "2026-09-28");
assert.ok(grid.includes("2026-10-31"));

for (const [raw, want] of [["9", "09:00"], ["930", "09:30"], ["0930", "09:30"], ["9:30", "09:30"], ["9.30", "09:30"], ["13", "13:00"], ["1315", "13:15"], [" 7:05 ", "07:05"]]) assert.equal(parseTime(raw), want, raw);
for (const raw of ["", "24", "9:5", "12:60", "abc", "12345"]) assert.equal(parseTime(raw), null, raw);
assert.equal(stepTime("23:50", 15), "00:05");
assert.equal(stepTime("00:10", -15), "23:55");

console.log("dates ok");
