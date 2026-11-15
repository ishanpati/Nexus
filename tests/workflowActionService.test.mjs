import { test } from "node:test";
import assert from "node:assert/strict";
import { createWorkflowActionCatalog } from "../src/services/workflowActionService.ts";

test("workflow catalog only requests backend metadata and accepts new IDs without UI registration", async () => {
  const descriptors = [{ id: "future.registered_action", schema_version: 2, parameters: [], workflow_compatible: true }];
  const calls = [];
  const load = createWorkflowActionCatalog(async command => { calls.push(command); return descriptors; });
  assert.equal(await load(), descriptors);
  assert.deepEqual(calls, ["list_workflow_actions"]);
});
