import assert from "node:assert/strict";
import { test } from "node:test";
import { parseFileHistory, withFileOpen, MAX_FILE_HISTORY, recordFileOpen, openedFiles } from "../src/fileUsage.ts";

test("file history is bounded, tolerant of bad data, and normalizes duplicates", () => {
  for (const raw of [null, "bad", "{}", "null"]) assert.deepEqual(parseFileHistory(raw), []);
  assert.deepEqual(parseFileHistory(JSON.stringify([null, 3, "", "C:\\a", "c:/A/", "C:\\b", "bad\npath"])), ["C:\\a", "C:\\b"]);
  const history = Array.from({ length: 300 }, (_, i) => `C:\\item${i}`);
  const next = withFileOpen(history, "C:\\item250");
  assert.equal(next.length, MAX_FILE_HISTORY);
  assert.equal(next[0], "C:\\item250");
  assert.equal(next.filter((item) => item === "C:\\item250").length, 1);
  assert.deepEqual(parseFileHistory(JSON.stringify(["文".repeat(1400)])), []);
});

test("successful file history persists locally and storage failure preserves session history", () => {
  const saved = new Map();
  globalThis.localStorage = { getItem: (key) => saved.get(key) ?? null, setItem: (key, value) => saved.set(key, value) };
  assert.equal(recordFileOpen("C:\\Desktop\\finance_tracker.xlsx"), null);
  assert.equal(openedFiles()[0], "C:\\Desktop\\finance_tracker.xlsx");
  assert.equal(saved.size, 1);
  globalThis.localStorage.setItem = () => { throw Error("denied"); };
  assert.match(recordFileOpen("C:\\Docs\\other.txt"), /session only/);
  assert.equal(openedFiles()[0], "C:\\Docs\\other.txt");
  delete globalThis.localStorage;
});
