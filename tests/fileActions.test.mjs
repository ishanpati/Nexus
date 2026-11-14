import assert from "node:assert/strict";
import { test } from "node:test";
import { canActivateFile, createFileActions } from "../src/fileActions.ts";

test("file opening sends the selected full path, a fixed action and no search text or arguments", async () => {
  const calls = [];
  const act = createFileActions(async (...args) => { calls.push(args); return { status: "opened", warning: null }; });
  const file = { full_path: "C:\\資料\\a & b.txt", filename: "a & b.txt", extension: "txt", is_directory: false };
  await act(file, "open"); await act(file, "reveal");
  assert.deepEqual(calls, [
    ["open_file_result", { request: { path: file.full_path, action: "open", confirmed: false } }],
    ["open_file_result", { request: { path: file.full_path, action: "reveal", confirmed: false } }],
  ]);
});

test("a confirmation response does not automatically open or confirm anything", async () => {
  const calls = [];
  const act = createFileActions(async (...args) => { calls.push(args); return { status: "confirmation_required", path: "C:\\script.ps1", message: "Confirm" }; });
  const outcome = await act({ full_path: "C:\\script.ps1" }, "open");
  assert.equal(calls.length, 1);
  // Confirmation captures this path, rather than whatever row later becomes selected.
  await act({ full_path: outcome.path }, "open", true);
  assert.equal(calls[1][1].request.path, "C:\\script.ps1");
  assert.equal(calls[1][1].request.confirmed, true);
});

test("stale, loading, or absent selections cannot activate file actions", () => {
  assert.equal(canActivateFile(true, "old", "new"), false);
  assert.equal(canActivateFile(false, "old", "new"), false);
  assert.equal(canActivateFile(false, "old", null), false);
  assert.equal(canActivateFile(false, "", ""), false);
  assert.equal(canActivateFile(false, "same", "same"), true);
});

test("native failures propagate and opened-with-warning remains distinguishable", async () => {
  const failed = createFileActions(async () => { throw "Access denied"; });
  await assert.rejects(failed({ full_path: "C:\\x" }, "open"), (error) => error === "Access denied");
  const opened = createFileActions(async () => ({ status: "opened", warning: "Could not hide" }));
  assert.deepEqual(await opened({ full_path: "C:\\x" }, "open"), { status: "opened", warning: "Could not hide" });
});
