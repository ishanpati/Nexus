import assert from "node:assert/strict";
import { test } from "node:test";
import { createFileSearchStore, searchMode, parentFolder, highlightParts, fileSearchError, FILE_BATCH_SIZE, FILE_RESULT_LIMIT } from "../src/fileSearch.ts";

function clock() {
  let now = 0;
  const tasks = new Set();
  return {
    timer(callback, delay) { const task = { at: now + delay, callback }; tasks.add(task); return () => tasks.delete(task); },
    advance(ms) { now += ms; for (const task of [...tasks]) if (task.at <= now) { tasks.delete(task); task.callback(); } },
  };
}
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const file = (name) => ({ full_path: `C:\\Docs\\${name}`, filename: name, extension: "txt", is_directory: false });

test("only f followed by a space activates file mode; normal app queries are preserved", () => {
  for (const input of ["", "f", "firefox", "file notes", "open notepad", " f name", "f\tname"]) assert.deepEqual(searchMode(input), { kind: "apps" });
  assert.deepEqual(searchMode("f "), { kind: "files", query: "" });
  assert.deepEqual(searchMode("F  meeting notes  "), { kind: "files", query: "meeting notes" });
  assert.deepEqual(searchMode('f ext:txt | "a"'), { kind: "files", query: 'ext:txt | "a"' });
});

test("debounces 150ms and sends only the latest query with a bounded result limit", async () => {
  const time = clock(); const calls = [];
  const store = createFileSearchStore(async (...args) => { calls.push(args); return []; }, time.timer);
  store.update("a"); time.advance(100); store.update("ab"); time.advance(149); await flush();
  assert.deepEqual(calls, []);
  time.advance(1); await flush();
  assert.deepEqual(calls, [["ab", FILE_RESULT_LIMIT]]);
  assert.equal(store.getSnapshot().loading, false);
  assert.equal(FILE_BATCH_SIZE, 10);
});

test("serializes backend requests and skips intermediate queries without stale success", async () => {
  const time = clock(); const calls = []; const requests = [];
  const store = createFileSearchStore((query) => { calls.push(query); const request = deferred(); requests.push(request); return request.promise; }, time.timer);
  store.update("old"); time.advance(150); await flush();
  store.update("middle"); time.advance(150); store.update("new"); time.advance(150); await flush();
  assert.deepEqual(calls, ["old"]);
  requests[0].resolve([file("old")]); await flush();
  assert.deepEqual(calls, ["old", "new"]);
  assert.deepEqual(store.getSnapshot().results, []);
  requests[1].resolve([file("new")]); await flush();
  assert.equal(store.getSnapshot().results[0].filename, "new");
  assert.equal(store.getSnapshot().resultQuery, "new");
});

test("retains previous matches while loading but clears them on a current error", async () => {
  const time = clock(); let next = deferred();
  const store = createFileSearchStore(() => next.promise, time.timer);
  store.update("first"); time.advance(150); next.resolve([file("first")]); await flush();
  next = deferred(); store.update("second");
  assert.equal(store.getSnapshot().results[0].filename, "first");
  assert.equal(store.getSnapshot().resultQuery, "first");
  assert.equal(store.getSnapshot().loading, true);
  time.advance(150); await flush(); next.reject({ code: "everything_unavailable" }); await flush();
  assert.deepEqual(store.getSnapshot().results, []);
  assert.match(store.getSnapshot().error, /Everything is unavailable/);
  assert.equal(store.getSnapshot().loading, false);
});

test("an old failure cannot overwrite a newer success, and the newest debounce still applies", async () => {
  const time = clock(); const old = deferred(); const calls = [];
  const store = createFileSearchStore((query) => { calls.push(query); return query === "old" ? old.promise : Promise.resolve([file(query)]); }, time.timer);
  store.update("old"); time.advance(150); await flush();
  store.update("new"); time.advance(50);
  old.reject({ code: "timeout" }); await flush();
  assert.equal(store.getSnapshot().error, "");
  assert.deepEqual(calls, ["old"]);
  time.advance(99); await flush(); assert.deepEqual(calls, ["old"]);
  time.advance(1); await flush();
  assert.deepEqual(calls, ["old", "new"]);
  assert.equal(store.getSnapshot().results[0].filename, "new");
});

test("leaving file mode cancels a scheduled request before the debounce fires", async () => {
  const time = clock(); let calls = 0;
  const store = createFileSearchStore(async () => { calls++; return []; }, time.timer);
  store.update("file query"); time.advance(100); store.update(null); time.advance(100); await flush();
  assert.equal(calls, 0);
  assert.equal(store.getSnapshot().loading, false);
});

test("late errors cannot replace a newer query or resurrect file mode after leaving", async () => {
  const time = clock(); const old = deferred(); let calls = 0;
  const store = createFileSearchStore(() => { calls++; return old.promise; }, time.timer);
  store.update("first"); time.advance(150); await flush();
  store.update("next"); store.update(null);
  old.reject({ code: "everything_unavailable" }); await flush(); time.advance(500); await flush();
  assert.equal(calls, 1);
  assert.deepEqual(store.getSnapshot(), { query: null, resultQuery: "", results: [], loading: false, error: "" });
});

test("leaving and re-entering the same query rejects the earlier generation", async () => {
  const time = clock(); const old = deferred(); let calls = 0;
  const store = createFileSearchStore(() => ++calls === 1 ? old.promise : Promise.resolve([file("fresh")]), time.timer);
  store.update("same"); time.advance(150); await flush();
  store.update(null); store.update("same"); time.advance(150); await flush();
  old.resolve([file("stale")]); await flush();
  assert.equal(store.getSnapshot().results[0].filename, "fresh");
  assert.equal(calls, 2);
});

test("blank queries and invalid inputs do not call Rust or show no-match errors", async () => {
  const time = clock(); const store = createFileSearchStore(() => { throw new Error("must not call"); }, time.timer);
  store.update(""); time.advance(150); await flush();
  assert.equal(store.getSnapshot().loading, false); assert.equal(store.getSnapshot().error, "");
  for (const query of ["x".repeat(1025), "a\nb", "a\0b", "日".repeat(342)]) {
    store.update(query); time.advance(150); await flush(); assert.match(store.getSnapshot().error, /1024/);
  }
});

test("no matches is a successful empty list; retry clears errors and can recover", async () => {
  const time = clock(); let fail = true;
  const store = createFileSearchStore(async () => { if (fail) throw { code: "es_not_found" }; return []; }, time.timer);
  store.update("query"); time.advance(150); await flush();
  assert.match(store.getSnapshot().error, /ES tool was not found/);
  fail = false; store.retry(); assert.equal(store.getSnapshot().error, ""); time.advance(150); await flush();
  assert.deepEqual(store.getSnapshot().results, []);
  assert.equal(store.getSnapshot().error, ""); assert.equal(store.getSnapshot().loading, false);
});

test("caps oversized responses and permits unsubscribe", async () => {
  const time = clock(); let updates = 0;
  const store = createFileSearchStore(async () => Array.from({ length: 110 }, (_, i) => file(String(i))), time.timer);
  const unsubscribe = store.subscribe(() => updates++);
  store.update("x"); time.advance(150); await flush();
  assert.equal(store.getSnapshot().results.length, 100);
  assert.equal(updates, 2); unsubscribe(); store.update(null); assert.equal(updates, 2);
});

test("parent labels preserve Unicode, root folders, UNC paths and directory suffixes", () => {
  assert.equal(parentFolder("C:\\资料\\a b.txt"), "C:\\资料");
  assert.equal(parentFolder("C:\\folder\\"), "C:\\");
  assert.equal(parentFolder("\\\\server\\share\\folder\\file.txt"), "\\\\server\\share\\folder");
  assert.equal(parentFolder("C:\\"), "C:\\");
});

test("highlighting is literal and preserves markup-like text and Unicode", () => {
  assert.deepEqual(highlightParts("A.txt a.TXT", ".txt"), [
    { text: "A", match: false }, { text: ".txt", match: true }, { text: " a", match: false }, { text: ".TXT", match: true },
  ]);
  for (const text of ["<img src=x>", "資料 résumé.txt", "İstanbul"]) {
    assert.equal(highlightParts(text, "i").map((p) => p.text).join(""), text);
  }
  assert.deepEqual(highlightParts("abc", ""), [{ text: "abc", match: false }]);
});

test("backend errors have clear unavailable, timeout, busy and fallback messages", () => {
  assert.match(fileSearchError({ code: "timeout" }), /too long/);
  assert.match(fileSearchError({ code: "busy" }), /another search/);
  assert.equal(fileSearchError({ code: "invalid_input", message: "Too long" }), "Too long");
  assert.match(fileSearchError("IPC failed"), /IPC failed/);
});
