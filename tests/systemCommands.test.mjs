import assert from "node:assert/strict";
import { test } from "node:test";
import { parseSystemCommand, suggestSystemCommands, systemCommandRegistry, isSystemAction, requiresSystemConfirmation, systemPreviewMessage } from "../src/systemCommands.ts";
import { searchCommandsAndApps } from "../src/commandSearch.ts";
import { indexApplications, indexSystemCommand, rankApplications } from "../src/appSearch.ts";

const apps = (...names) => indexApplications(names.map((display_name) => ({ id: display_name, display_name, launch: { method: "shell_app" }, sources: [] })));
const action = (query) => {
  const parsed = parseSystemCommand(query);
  assert.equal(parsed.status, "ready", query);
  assert.equal(isSystemAction(parsed.action), true);
  return parsed.action;
};

test("volume grammar returns typed absolute and relative actions with normalized names/whitespace", () => {
  for (const query of ["volume 35", "vol 35", "  VoLuMe\t 35  ", "VOL\n35"]) {
    assert.deepEqual(action(query), { id: "set_volume", args: { percent: 35 } });
  }
  for (const percent of [0, 1, 99, 100]) assert.equal(action(`volume ${percent}`).args.percent, percent);
  for (const [input, delta] of [["+10", 10], ["-10", -10], ["up", 10], ["down", -10], ["+100", 100], ["-100", -100], ["-0", 0]]) {
    assert.deepEqual(action(`vol ${input}`), { id: "adjust_volume", args: { delta } });
  }
});

test("static commands and reasonable aliases map only to closed typed actions", () => {
  const cases = [
    ["mute", { id: "set_mute", args: { muted: true } }],
    ["unmute", { id: "set_mute", args: { muted: false } }],
    ...["sound", "wifi", "bluetooth", "display"].map((page) => [`${page} settings`, { id: "open_settings", args: { page } }]),
    ["lock", { id: "lock", args: {} }], ["sleep", { id: "sleep", args: {} }],
  ];
  for (const [query, expected] of cases) assert.deepEqual(action(query), expected);
  for (const [alias, canonical] of [["audio settings", "sound settings"], ["wi-fi settings", "wifi settings"], ["bt settings", "bluetooth settings"], ["screen settings", "display settings"], ["lock screen", "lock"], ["sleep computer", "sleep"]]) {
    assert.deepEqual(action(alias), action(canonical));
  }
  const aliases = systemCommandRegistry.flatMap((entry) => entry.names);
  assert.equal(new Set(aliases).size, aliases.length);
  assert.equal(new Set(systemCommandRegistry.map((entry) => entry.id)).size, systemCommandRegistry.length);
});

test("malformed numeric and injection-like input never yields an action", () => {
  for (const argument of ["NaN", "Infinity", "-Infinity", "1e2", "0x20", "3.5", "35%", "35abc", "101", "+101", "-101", "++10", "--10", "+ 10", "35 40", "35;calc", "35 && calc", "9".repeat(500)]) {
    const result = parseSystemCommand(`volume ${argument}`);
    assert.equal(result.status, "invalid", argument);
    assert.equal(result.action, null);
    assert.ok(result.description.length);
  }
  for (const input of ["mute true", "sleep now", "lock && calc", "sound settings ms-settings:other", "volume 3\0"]) {
    const result = parseSystemCommand(input);
    assert.equal(result.status, "invalid", input);
    assert.equal(result.action, null);
  }
});

test("typed-object validation rejects non-finite values, coercion and unknown arguments", () => {
  for (const value of [NaN, Infinity, -Infinity, "35", null, true, 3.5, -1, 101]) assert.equal(isSystemAction({ id: "set_volume", args: { percent: value } }), false);
  for (const value of [NaN, Infinity, -Infinity, 101, -101, "10"]) assert.equal(isSystemAction({ id: "adjust_volume", args: { delta: value } }), false);
  for (const value of [null, [], {}, { id: "sleep", args: { confirmed: true } }, { id: "sleep", args: {}, shell: "cmd" }, { id: "open_settings", args: { page: "ms-settings:whatever" } }, { id: "set_mute", args: { muted: 1 } }, { id: "unknown", args: {} }]) assert.equal(isSystemAction(value), false);
});

test("incomplete input gets context; unknown, fuzzy or empty text never infers system actions", () => {
  for (const query of ["vol", "volume", "volume +", "volume -"]) {
    const suggestions = suggestSystemCommands(query);
    assert.ok(suggestions.length);
    assert.ok(suggestions.every((item) => item.status === "incomplete" && item.action === null));
  }
  for (const query of ["", "  ", "slleep", "sleepy", "volcano", "volume35", "calc", "C:\\sleep.exe"]) assert.deepEqual(suggestSystemCommands(query), []);
  assert.equal(suggestSystemCommands("volume NaN")[0].status, "invalid");
  assert.match(systemPreviewMessage(suggestSystemCommands("volume")[0]), /0–100/);
});

test("sleep is always confirmation-required metadata", () => {
  for (const input of ["sleep", "sleep computer", "SLEEP"]) {
    const parsed = parseSystemCommand(input);
    assert.equal(parsed.requiresConfirmation, true);
    assert.equal(requiresSystemConfirmation(parsed.action), true);
    assert.match(systemPreviewMessage(parsed), /Explicit confirmation/);
  }
  assert.equal(parseSystemCommand("lock").requiresConfirmation, false);
  assert.deepEqual(parseSystemCommand("volume 35"), parseSystemCommand("volume 35"));
});

test("complete command wins equal-name collision but keeps app selectable regardless of usage", () => {
  for (const query of ["mute", "sleep", "lock", "sound settings", "volume 35"]) {
    const result = searchCommandsAndApps(query, apps(query), { ["app:" + query]: 1e9 });
    assert.equal(result[0].kind, "system", query);
    assert.equal(result[1].kind, "app", query);
  }
  assert.equal(searchCommandsAndApps("volume", apps("Volume"))[0].kind, "app");
  assert.equal(searchCommandsAndApps("sound", apps("Sound"))[0].kind, "app");
  assert.equal(searchCommandsAndApps("volume NaN", apps("Volume NaN"))[0].kind, "app");
});

test("file mode is exclusive and existing app/Notepad ranking remains intact", () => {
  for (const query of ["f sleep", "F volume 35", "f "]) assert.deepEqual(searchCommandsAndApps(query, apps("Sleep")), []);
  for (const query of ["notepad", "open notepad", "launch notepad", "note"]) {
    assert.equal(searchCommandsAndApps(query, apps("Notepad"))[0].command.id, "open_notepad");
  }
  for (const query of ["", "chorme", "editor", "zzzzzzzz"]) {
    const index = apps("Google Chrome", "Editor", "Editor Plus");
    assert.deepEqual(searchCommandsAndApps(query, index).filter((item) => item.kind === "app"), rankApplications(index, query));
  }
  const stale = indexSystemCommand(suggestSystemCommands("sleep")[0]);
  assert.deepEqual(rankApplications([stale], "slleep"), []);
  assert.deepEqual(rankApplications([stale], ""), []);
});
