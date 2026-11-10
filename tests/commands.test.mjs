import assert from "node:assert/strict";
import { test } from "node:test";
import { commandRegistry, findCommands } from "../src/commands.ts";

test("matches the command label and both aliases", () => {
  for (const query of ["open notepad", "notepad", "launch notepad"]) {
    assert.deepEqual(findCommands(query).map(command => command.id), ["open_notepad"]);
  }
});

test("normalizes case, surrounding whitespace, and repeated spaces", () => {
  assert.equal(findCommands("  LAUNCH\t  NoTePaD  ")[0]?.id, "open_notepad");
});

test("supports partial searches and empty-input suggestions", () => {
  assert.equal(findCommands("note")[0]?.id, "open_notepad");
  assert.equal(findCommands("   ").length, 1);
});

test("does not turn paths, command suffixes, or unknown apps into results", () => {
  for (const query of ["calculator", "notepad.exe", "C:\\Windows\\notepad.exe", "notepad & calc", "open notepad file.txt"]) {
    assert.deepEqual(findCommands(query), []);
  }
});

test("registry exposes only a fixed ID and an empty native argument object", () => {
  assert.deepEqual(Object.keys(commandRegistry), ["open_notepad"]);
  assert.deepEqual(commandRegistry.open_notepad.request, { id: "open_notepad", args: {} });
});
