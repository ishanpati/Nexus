import assert from "node:assert/strict";
import { test } from "node:test";
import { searchCommandsAndApps } from "../src/commandSearch.ts";
import { dispatchResult, executeWithLifecycle } from "../src/resultExecution.ts";
import { needsDiscardConfirmation } from "../src/studioNavigation.ts";
import { createStudioService } from "../src/services/workflowStudioService.ts";

test("partial workflow matches dispatch the selected stable command ID", async () => {
  for (const [query, id] of [["cre", "create_workflow"], ["edit work", "edit_workflow"]]) {
    const calls = [];
    await dispatchResult(searchCommandsAndApps(query, [])[0], { command: async id => calls.push(id) });
    assert.deepEqual(calls, [id]);
  }
});
test("Studio failure retains launcher query and does not hide; success clears then hides", async () => {
  for (const fail of [true, false]) {
    const calls = [];
    await executeWithLifecycle(async () => {
      calls.push("open");
      if (fail) throw new Error("Studio failed to initialize");
      return { status: "success" };
    }, { reset: () => calls.push("clear"), restore: () => calls.push("restore"),
      hide: async () => calls.push("hide"), error: () => calls.push("error") });
    assert.deepEqual(calls, fail ? ["open", "error"] : ["open", "clear", "hide"]);
  }
});
test("dirty drafts intercept create, list, stable-ID edit, and native close", () => {
  for (const intent of [{ kind: "close" }, ...[{ mode: "create" }, { mode: "list" }, { mode: "edit", id: "saved-id" }].map(route => ({ kind: "navigate", route }))]) {
    assert.equal(needsDiscardConfirmation(true, { sequence: 1, intent }), true);
    assert.equal(needsDiscardConfirmation(false, { sequence: 1, intent }), false);
  }
});
test("repository reads and navigation use explicit IDs and never workflow display names", async () => {
  const calls = [];
  const service = createStudioService(async (command, args) => { calls.push([command, args]); return []; });
  await service.list(); await service.get("stable-id"); await service.open({ mode: "edit", id: "stable-id" });
  assert.deepEqual(calls, [["list_workflows", undefined], ["get_workflow", { id: "stable-id" }],
    ["open_workflow_studio", { route: { mode: "edit", id: "stable-id" } }]]);
});
