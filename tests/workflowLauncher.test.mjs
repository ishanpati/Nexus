import assert from "node:assert/strict";
import { test } from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import ts from "typescript";
import * as runService from "../src/services/workflowRunService.ts";
import { searchMode } from "../src/fileSearch.ts";
import { searchCommandsAndApps } from "../src/commandSearch.ts";
import { indexApplications, moveSelection } from "../src/appSearch.ts";
import { createWorkflowSearchStore, searchWorkflows } from "../src/workflowSearch.ts";
import { dispatchResult, executeWithLifecycle, dismissLauncher, summonLauncher } from "../src/resultExecution.ts";
import { createWorkflowRuns, runActive } from "../src/services/workflowRunService.ts";

const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const saved = (id, name, aliases = []) => ({ workflow: { id, name, aliases, description: "Local setup", steps: [{ id: "step", action_id: "app.launch" }], revision: 1 }, executable: true, issues: [] });
const workflows = [saved("coding-id", "Coding Mode", ["dev space"]), saved("audio-id", "Audio Setup"), saved("lock-id", "Lock", ["create workflow", "f notes"])];
const run = (overrides = {}) => ({ run_id: "run-1", workflow_id: "coding-id", sequence: 0, workflow_revision: 1, status: "pending", cancel_requested: false,
  action_in_flight: false, steps: [], confirmation: null, error: null, ...overrides });
const tick = async () => { for (let i = 0; i < 6; i++) await Promise.resolve(); };

test("w-space lists workflows; partial names and aliases use existing case-insensitive tiers", () => {
  assert.equal(searchWorkflows("w ", workflows).length, 3);
  for (const query of ["w cod", "W COD", "w coding mode", "w dev", "w DEV SPACE", "w space"]) {
    assert.equal(searchWorkflows(query, workflows)[0].workflow.id, "coding-id", query);
  }
  for (const query of ["w", "workflow", " w cod", "w\tcod", "cod"]) assert.deepEqual(searchWorkflows(query, workflows), []);
  assert.deepEqual(searchWorkflows("w no match", workflows), []);
});

test("exclusive prefixes resolve collisions; ordinary commands/apps and f mode remain separate", () => {
  const apps = indexApplications([{ id: "app", display_name: "Lock", launch: { method: "packaged_app" } }]);
  for (const query of ["w ", "w lock", "w create workflow", "f notes"]) assert.deepEqual(searchCommandsAndApps(query, apps), []);
  assert.equal(searchWorkflows("w create workflow", workflows)[0].workflow.id, "lock-id");
  assert.equal(searchWorkflows("w f notes", workflows)[0].workflow.id, "lock-id");
  assert.deepEqual(searchMode("F  notes"), { kind: "files", query: "notes" });
  assert.equal(searchCommandsAndApps("lock", apps)[0].kind, "system");
  assert.equal(searchCommandsAndApps("lock", apps)[1].app.id, "app");
  assert.equal(searchCommandsAndApps("create workflow", [])[0].command.id, "create_workflow");
  assert.equal(searchCommandsAndApps("edit workflow", [])[0].command.id, "edit_workflow");
});

test("different keyboard highlight and clicking that row dispatch exactly the same workflow ID, never the query", async () => {
  const rows = searchWorkflows("w ", workflows); const calls = [];
  const highlighted = rows[moveSelection(0, 1, rows.length)];
  assert.notEqual(rows[0].workflow.id, highlighted.workflow.id);
  assert.equal(calls.length, 0); // Search and preview have no executor.
  const handlers = { workflow: async id => { calls.push(id); } };
  await dispatchResult(highlighted, handlers);
  await dispatchResult(rows.find(row => row.key === highlighted.key), handlers);
  assert.deepEqual(calls, [highlighted.workflow.id, highlighted.workflow.id]);
});

test("repository reload reflects rename/deletion and suppresses stale reads/errors", async () => {
  const requests = []; const store = createWorkflowSearchStore(() => { const next = deferred(); requests.push(next); return next.promise; });
  const old = store.refresh(); const latest = store.refresh();
  requests[1].resolve([saved("coding-id", "Renamed")]); await latest;
  requests[0].resolve(workflows); await old;
  assert.equal(searchWorkflows("w cod", store.getSnapshot().items).length, 0);
  assert.equal(searchWorkflows("w ren", store.getSnapshot().items)[0].workflow.id, "coding-id");
  const stale = store.refresh(); const deletion = store.refresh();
  assert.equal(store.getSnapshot().loading, true);
  requests[3].resolve([]); await deletion; requests[2].reject(new Error("old")); await stale;
  assert.deepEqual(store.getSnapshot(), { items: [], loading: false, error: "" });
  const failed = store.refresh(); requests[4].reject({ message: "Database unavailable" }); await failed;
  assert.equal(store.getSnapshot().error, "Database unavailable");
});

test("synchronous start gate rejects rapid Enter/click launches and active runs, with ID-only IPC", async () => {
  const start = deferred(); const calls = [];
  const store = createWorkflowRuns(async (command, args) => {
    calls.push([command, args]); if (command === "start_workflow") return start.promise;
    return run({ status: "running", sequence: 2 });
  });
  const first = store.start("coding-id");
  await assert.rejects(store.start("coding-id"), /active run/);
  start.resolve(run()); await first; await tick();
  await assert.rejects(store.start("coding-id"), /active run/);
  assert.deepEqual(calls, [["start_workflow", { workflowId: "coding-id" }], ["workflow_run_status", { runId: "run-1" }]]);
});

test("start acceptance clears input independently of progress; failed start retains query and recovers runs without retry", async () => {
  for (const fail of [false, true]) {
    const calls = []; let query = "w cod";
    const store = createWorkflowRuns(async command => {
      calls.push(command);
      if (command === "list_workflow_runs") return [run({ sequence: 3, status: "failed", error: { code: "not_found", message: "Deleted" } })];
      if (fail) throw { message: "Start response lost" };
      return run();
    });
    let error = "";
    await executeWithLifecycle(async () => { await store.start("coding-id"); return { status: "success" }; }, {
      reset: () => { query = ""; }, restore: () => {}, hide: async () => {}, error: message => { error = message; },
    });
    assert.equal(query, fail ? "w cod" : "");
    if (fail) { assert.match(error, /Start response lost.*Check workflow runs/); assert.equal(store.getSnapshot().runs[0].status, "failed"); }
    assert.equal(calls.filter(command => command === "start_workflow").length, 1);
    await dismissLauncher(() => { query = ""; }, async () => {});
    summonLauncher(() => { query = ""; }, () => {});
    assert.equal(store.getSnapshot().runs.length, 1); // No cancel on reset, hide, or summon.
    assert.ok(!calls.includes("cancel_workflow_run"));
  }
});

test("events before start responses and out-of-order list/status reads cannot regress progress", async () => {
  const start = deferred(); const list = deferred(); const status = deferred();
  const store = createWorkflowRuns(async command => command === "start_workflow" ? start.promise : command === "list_workflow_runs" ? list.promise : status.promise);
  const starting = store.start("coding-id"); const listing = store.refresh();
  const updating = store.event({ run_id: "run-1", sequence: 4 });
  status.resolve(run({ sequence: 4, status: "succeeded" })); await updating;
  start.resolve(run()); await starting;
  list.resolve([run({ sequence: 1, status: "running" })]); await listing;
  assert.equal(store.getSnapshot().runs[0].status, "succeeded");
  assert.equal(store.getSnapshot().runs[0].sequence, 4);
  assert.equal(runActive(store.getSnapshot().runs[0]), false);
});

test("confirmation sends exact run/step/token only after explicit approval; cancel/decide share a duplicate gate", async () => {
  const prompt = { token: "one-use", step_id: "terminal-step", action: { action_id: "system.lock", parameters: {} }, message: "Lock this computer", expires_in_ms: 60000 };
  const calls = []; let next = deferred();
  const store = createWorkflowRuns(async (command, args) => {
    calls.push([command, args]);
    if (command === "list_workflow_runs") return [run({ status: "confirmation-required", confirmation: prompt, sequence: 3 })];
    return next.promise;
  });
  await store.refresh(); assert.equal(calls.length, 1);
  const approval = store.decide("run-1", prompt, "approve");
  await store.decide("run-1", prompt, "approve"); await store.cancel("run-1");
  assert.deepEqual(calls[1], ["decide_workflow_action", { runId: "run-1", stepId: "terminal-step", token: "one-use", decision: "approve" }]);
  assert.equal(calls.length, 2);
  next.resolve(run({ sequence: 4, status: "running" })); await approval;
  next = deferred(); const cancelling = store.cancel("run-1");
  next.resolve(run({ sequence: 5, cancel_requested: true, action_in_flight: true, status: "running" })); await cancelling;
  assert.deepEqual(calls[2], ["cancel_workflow_run", { runId: "run-1" }]);
  assert.equal(store.getSnapshot().runs[0].cancel_requested, true);
  next = deferred(); const denial = store.decide("run-1", prompt, "deny"); next.reject({ message: "Approval expired" }); await denial;
  assert.equal(store.getSnapshot().error, "Approval expired");
});

test("progress renders completed/current/failed steps, exact approval targets and truthful app acceptance", () => {
  const source = readFileSync(new URL("../src/components/WorkflowProgress.tsx", import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, { compilerOptions: { jsx: ts.JsxEmit.ReactJSX, module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2021 } }).outputText;
  const module = { exports: {} }; const require = createRequire(import.meta.url);
  new Function("require", "exports", compiled)(id => id === "../services/workflowRunService" ? runService : require(id), module.exports);
  const { WorkflowProgress } = module.exports;
  const render = current => renderToStaticMarkup(WorkflowProgress({ runs: [current], saved: workflows, selectedId: current.run_id, busy: [], error: "",
    onSelect() {}, onClose() {}, onCancel() {}, onDecide() {}, onRefresh() {} }));
  const steps = [{ step_id: "first", action_id: "app.launch", status: "succeeded", output: { accepted: true, readiness_verified: false }, error: null },
    { step_id: "second", action_id: "system.lock", status: "running", output: null, error: null }];
  assert.match(render(run({ steps, status: "running" })), /1\/2 steps completed.*Current step/s);
  assert.match(render(run({ steps, status: "running", action_in_flight: true })), /cannot be recalled/);
  const complete = render(run({ steps: steps.map(step => ({ ...step, status: "succeeded" })), status: "succeeded" }));
  assert.match(complete, /Completed/); assert.match(complete, /readiness unverified/);
  assert.doesNotMatch(complete, /fully loaded|Cancel workflow|Approve system.lock/);
  const failure = { message: "Target disappeared", code: "invalid_reference", details: null };
  const failed = render(run({ steps: [steps[0], { ...steps[1], status: "failed", error: failure }], status: "failed", error: failure }));
  assert.match(failed, /Step 2: Target disappeared/); assert.match(failed, /Failed/);
  const prompt = { token: "secret-token", step_id: "second", message: "Lock this computer?", expires_in_ms: 30000,
    action: { action_id: "system.lock", schema_version: 1, parameters: { target: "this computer" } } };
  const approval = render(run({ steps, status: "confirmation-required", confirmation: prompt }));
  for (const text of ["Approval required", "Deny action", "Approve system.lock", "this computer", "second", "run-1", "30 seconds"]) assert.ok(approval.includes(text), text);
  assert.ok(!approval.includes("secret-token"));
  const cancelled = render(run({ status: "cancelled" })); assert.match(cancelled, /Cancelled/); assert.doesNotMatch(cancelled, /Cancel workflow/);
});

test("background reads coalesce while pending and keep active runs when terminal history is bounded", async () => {
  const pending = deferred(); let calls = 0;
  const store = createWorkflowRuns(async () => { calls++; return pending.promise; });
  const first = store.refresh(); const second = store.refresh();
  assert.equal(calls, 1);
  pending.resolve([run({ status: "running" }), ...Array.from({ length: 128 }, (_, i) => run({ run_id: `terminal-${i}`, status: "succeeded" }))]);
  await Promise.all([first, second]);
  assert.equal(store.getSnapshot().runs.length, 128);
  assert.equal(store.getSnapshot().runs.find(item => item.run_id === "run-1").status, "running");
  assert.equal(store.getSnapshot().runs.find(item => item.run_id === "terminal-0"), undefined);
});
