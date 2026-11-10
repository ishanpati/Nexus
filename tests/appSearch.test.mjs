import assert from "node:assert/strict";
import { test } from "node:test";
import { indexApplications, indexCommand, rankApplications, moveSelection, visibleResultCount } from "../src/appSearch.ts";
import { findCommands } from "../src/commands.ts";
import { parseLaunchCounts, recordSuccessfulLaunch } from "../src/launchUsage.ts";

const app = (display_name, id = display_name) => ({ id, display_name, launch: { method: "shell_app" }, sources: ["apps_folder"] });
const names = (results) => results.map((item) => item.label);

test("ranking follows exact, name prefix, word prefix, substring, then typo", () => {
  const index = indexApplications([app("Znotepad"), app("Notepod"), app("My Notepad"), app("Notepad Pro"), app("Notepad")]);
  assert.deepEqual(names(rankApplications(index, "notepad")), ["Notepad", "Notepad Pro", "My Notepad", "Znotepad", "Notepod"]);
});

test("normalization and curated exact aliases work without using paths", () => {
  const index = indexApplications([app("Visual Studio Code"), app("Google Chrome"), app("Calculator")]);
  assert.equal(rankApplications(index, "  VSCode  ")[0]?.label, "Visual Studio Code");
  assert.equal(rankApplications(index, "VISUAL\t STUDIO CODE")[0]?.label, "Visual Studio Code");
  assert.equal(rankApplications(index, "calc")[0]?.label, "Calculator");
  assert.deepEqual(rankApplications(index, "C:\\Calculator.exe"), []);
});

test("fuzzy accepts limited edits and transposition but rejects nonsense and short typos", () => {
  const index = indexApplications([app("Notepad"), app("Google Chrome"), app("Calculator"), app("Paint")]);
  for (const query of ["notepda", "notepd", "notepadd"]) assert.equal(rankApplications(index, query)[0]?.label, "Notepad");
  assert.equal(rankApplications(index, "chorme")[0]?.label, "Google Chrome");
  assert.equal(rankApplications(index, "calclator")[0]?.label, "Calculator");
  for (const query of ["xyz", "npt", "zzzzzzzz", "notepad & calc", "notepad file.txt", "x".repeat(5000)]) {
    assert.deepEqual(rankApplications(index, query), [], query);
  }
});

test("frequency only breaks text ties, with a capped bonus and deterministic fallbacks", () => {
  const index = indexApplications([app("Notepad"), app("Notepad Alpha"), app("Notepad Zulu"), app("Notepod")]);
  const counts = { "app:Notepod": 1e12, "app:Notepad Zulu": 20 };
  assert.deepEqual(names(rankApplications(index, "notepad", counts)), ["Notepad", "Notepad Zulu", "Notepad Alpha", "Notepod"]);
  assert.deepEqual(rankApplications(index, "", { "app:Notepad": NaN }), rankApplications(index, ""));
});

test("equal fuzzy matches prefer fewer edits even with extreme usage", () => {
  const index = indexApplications([app("Calculatr"), app("Calculatrx")]);
  assert.deepEqual(names(rankApplications(index, "calculator", { "app:Calculatrx": 1e12 })), ["Calculatr", "Calculatrx"]);
});

test("v0.1 commands and partial aliases remain executable ahead of equally matching previews", () => {
  const apps = indexApplications([app("Notepad")]);
  for (const query of ["", "open notepad", "notepad", "launch notepad", "launch note", "note"]) {
    const result = rankApplications([...findCommands(query).map(indexCommand), ...apps], query);
    assert.equal(result[0]?.kind, "command", query);
    assert.equal(result[0]?.command.id, "open_notepad");
  }
  const result = rankApplications(apps, "notepad")[0];
  assert.equal(result.kind, "app");
  assert.equal(result.command, undefined);
});

test("all matches remain available beyond the initial eight with safe wraparound", () => {
  const result = rankApplications(indexApplications(Array.from({ length: 24 }, (_, i) => app(`Editor ${i}`))), "editor");
  assert.equal(result.length, 24);
  assert.equal(visibleResultCount(8, 0, 24), 8);
  assert.equal(visibleResultCount(8, 8, 24), 16);
  assert.equal(visibleResultCount(8, 23, 24), 24);
  assert.equal(visibleResultCount(24, 0, 2), 2);
  assert.equal(moveSelection(0, -1, 24), 23);
  assert.equal(moveSelection(23, 1, 24), 0);
  assert.equal(moveSelection(0, 1, 0), 0);
});

test("usage tolerates missing or malformed storage and counts only explicit successes", () => {
  for (const value of [null, "bad JSON", "[]", "null", "123"]) assert.deepEqual(parseLaunchCounts(value), {});
  assert.deepEqual(parseLaunchCounts('{"app:a":4,"app:b":-1,"app:c":"9","random":12}'), { "app:a": 4 });
  const old = { "app:a": 4 };
  assert.deepEqual(recordSuccessfulLaunch(old, "command:open_notepad"), { "app:a": 4, "command:open_notepad": 1 });
  assert.deepEqual(old, { "app:a": 4 });
});
