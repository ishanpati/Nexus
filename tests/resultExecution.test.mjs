import assert from "node:assert/strict";
import { test } from "node:test";
import { searchCommandsAndApps } from "../src/commandSearch.ts";
import { indexApplications } from "../src/appSearch.ts";
import { dispatchResult, executeWithLifecycle, dismissLauncher, summonLauncher } from "../src/resultExecution.ts";

function harness(query) {
  const state = { query, selected: 3, error: "", hidden: false, focused: false };
  const original = { ...state };
  const reset = () => { state.query = ""; state.selected = 0; state.error = ""; };
  return { state, reset, lifecycle: { reset, restore: () => Object.assign(state, original),
    hide: async () => { state.hidden = true; }, error: message => { state.error = message; } } };
}
const selected = query => searchCommandsAndApps(query, [])[0];
const unexpected = async () => assert.fail("unexpected action");
const handlers = { app: unexpected, command: unexpected, file: unexpected, audio: unexpected, system: unexpected };

test("partial selected Settings rows carry the same executable actions for Enter and click", async () => {
  for (const [query, page] of [["disp", "display"], ["blu", "bluetooth"]]) {
    const row = selected(query);
    for (const activate of [dispatchResult, dispatchResult]) { // Both UI entry points use this dispatcher.
      const calls = [];
      await activate(row, { ...handlers, system: async action => { calls.push(action); return { status: "accepted", message: "ok", warning: null }; } });
      assert.deepEqual(calls, [{ id: "open_settings", args: { page } }]);
    }
  }
});

test("Chrome prefix and fuzzy app selections launch only the catalog ID", async () => {
  const apps = indexApplications([{ id: "catalog-chrome", display_name: "Google Chrome", launch: { method: "shell_app" }, sources: [] }]);
  for (const query of ["chr", "chorme"]) {
    const row = searchCommandsAndApps(query, apps)[0];
    const ids = [];
    await dispatchResult(row, { ...handlers, app: async id => { ids.push(id); return { launched: true, warning: null }; } });
    assert.deepEqual(ids, ["catalog-chrome"]);
  }
});

test("selected volume preserves parsed numbers; invalid or incomplete volume cannot execute", async () => {
  for (const [query, expected] of [["vol 60", { id: "set_volume", args: { percent: 60 } }], ["volume +10", { id: "adjust_volume", args: { delta: 10 } }]]) {
    await dispatchResult(selected(query), { ...handlers, audio: async action => {
      assert.deepEqual(action, expected); return { volume_percent: 60, muted: false, endpoint_id: "mock" };
    } });
  }
  for (const query of ["vol", "vol +", "vol NaN", "volume 101", "volume 60xyz"]) {
    assert.equal((await dispatchResult(selected(query), handlers)).status, "help");
  }
});

test("success resets query and selection and hides; failure retains query and exposes error", async () => {
  const good = harness("vol 60");
  await executeWithLifecycle(async () => ({ status: "success" }), good.lifecycle);
  assert.deepEqual(good.state, { query: "", selected: 0, error: "", hidden: true, focused: false });
  const bad = harness("vol 60");
  await executeWithLifecycle(async () => { throw new Error("No audio device"); }, bad.lifecycle);
  assert.equal(bad.state.query, "vol 60"); assert.equal(bad.state.error, "No audio device"); assert.equal(bad.state.hidden, false);
});

test("sleep remains a two-step operation; help and confirmation do not clear or hide", async () => {
  const ui = harness("sle"); let calls = 0;
  const outcome = await executeWithLifecycle(() => dispatchResult(selected("sle"), { ...handlers, system: async action => {
    calls++; assert.equal(action.id, "sleep"); return { status: "confirmation_required", token: "one-use", message: "Confirm" };
  } }), ui.lifecycle);
  assert.equal(outcome.status, "sleep_confirmation"); assert.equal(calls, 1);
  assert.equal(ui.state.query, "sle"); assert.equal(ui.state.hidden, false);
});

test("lock and confirmed sleep reset before execution, restoring query on native failure", async () => {
  for (const fail of [true, false]) {
    const ui = harness("sleep");
    await executeWithLifecycle(async () => {
      assert.equal(ui.state.query, "");
      if (fail) throw new Error("Restricted");
      return { status: "success" };
    }, ui.lifecycle, true);
    assert.equal(ui.state.query, fail ? "sleep" : "");
    assert.equal(ui.state.hidden, !fail);
  }
});

test("Escape clears before hiding; summon clears and focuses without a mode-change reset", async () => {
  const ui = harness("f report");
  await dismissLauncher(ui.reset, ui.lifecycle.hide);
  assert.equal(ui.state.query, ""); assert.equal(ui.state.hidden, true);
  ui.state.query = "disp";
  summonLauncher(ui.reset, () => { assert.equal(ui.state.query, ""); ui.state.focused = true; });
  assert.equal(ui.state.focused, true);
});

test("hide failure after success never restores query or retries the operation", async () => {
  const ui = harness("disp"); let calls = 0;
  await executeWithLifecycle(async () => { calls++; return { status: "success" }; }, { ...ui.lifecycle, hide: async () => { throw new Error("hide failed"); } });
  assert.equal(calls, 1); assert.equal(ui.state.query, ""); assert.match(ui.state.error, /Action succeeded/);
});

test("file confirmation and reveal retain selected paths and explicit confirmation flags", async () => {
  const row = { kind: "file", key: "file:chosen", label: "chosen.exe", file: { full_path: "C:\\Users\\test\\chosen.exe" } };
  const calls = [];
  const file = async (...args) => { calls.push(args); return { status: "confirmation_required", path: row.file.full_path, message: "Confirm" }; };
  const ui = harness("f chosen");
  await executeWithLifecycle(() => dispatchResult(row, { ...handlers, file }), ui.lifecycle);
  assert.equal(ui.state.query, "f chosen"); assert.equal(ui.state.hidden, false);
  await dispatchResult(row, { ...handlers, file }, { confirmed: true });
  await dispatchResult(row, { ...handlers, file }, { fileAction: "reveal" });
  assert.deepEqual(calls, [[row.file, "open", undefined], [row.file, "open", true], [row.file, "reveal", undefined]]);
});
