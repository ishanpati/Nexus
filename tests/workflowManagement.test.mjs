import assert from "node:assert/strict";
import { test } from "node:test";
import { createStudioNavigation } from "../src/studioNavigation.ts";
import { createStudioService } from "../src/services/workflowStudioService.ts";
import { filterWorkflows } from "../src/workflowList.ts";
import { createDraft, draftFingerprint, validateDraft, reorderSteps } from "../src/workflowDraft.ts";

const tick = () => new Promise(resolve => setImmediate(resolve));
const request = (sequence, intent = { kind: "close" }) => ({ sequence, intent });
function navigationHarness() {
  const h = { dirty: true, calls: [], state: null, save: async () => true, commit: async () => {} };
  h.navigation = createStudioNavigation({ isDirty: () => h.dirty,
    save: async () => { h.calls.push("save"); const ok = await h.save(); if (ok) h.dirty = false; return ok; },
    commit: async (r, accept) => { h.calls.push(["commit", r.sequence, accept]); await h.commit(); },
    accepted: r => h.calls.push(["accepted", r.sequence]), changed: state => { h.state = state; } });
  return h;
}

test("Save / Discard / Cancel guard every editor route and Studio close", async () => {
  const intents = [{ kind: "close" }, { kind: "quit" }, ...[{ mode: "create" }, { mode: "list" }, { mode: "edit", id: "other" }].map(route => ({ kind: "navigate", route }))];
  for (const intent of intents) for (const choice of ["save", "discard", "cancel"]) {
    const h = navigationHarness(), r = request(1, intent);
    h.navigation.receive(r);
    assert.equal(h.state.pending, r); assert.deepEqual(h.calls, []);
    await h.navigation.choose(choice);
    assert.deepEqual(h.calls, [...(choice === "save" ? ["save"] : []), ["commit", 1, choice !== "cancel"], ...(choice !== "cancel" ? [["accepted", 1]] : [])]);
    assert.equal(h.dirty, choice !== "save");
    assert.equal(h.state.pending, null);
    h.navigation.receive(r); // A late event cannot replay navigation.
    assert.equal(h.state.pending, null);
  }
});

test("navigation waits for successful persistence and rejects duplicate Save and Escape", async () => {
  const h = navigationHarness(); let finish;
  h.save = () => new Promise(resolve => { finish = resolve; });
  h.navigation.receive(request(1));
  const saving = h.navigation.choose("save");
  h.navigation.setSaving(true);
  await h.navigation.choose("save"); await h.navigation.choose("cancel");
  h.navigation.receive(request(1));
  assert.deepEqual(h.calls, ["save"]); assert.equal(h.state.busy, true);
  h.navigation.setSaving(false); finish(true); await saving;
  assert.deepEqual(h.calls, ["save", ["commit", 1, true], ["accepted", 1]]);
});

test("validation, stale revision, deleted workflow and commit failures preserve the editor", async () => {
  for (const failure of [false, new Error("revision_conflict"), new Error("Workflow was deleted")]) {
    const h = navigationHarness();
    h.save = async () => { if (failure instanceof Error) throw failure; return failure; };
    h.navigation.receive(request(1)); await h.navigation.choose("save");
    assert.equal(h.state.pending.sequence, 1); assert.equal(h.dirty, true);
    assert.deepEqual(h.calls, ["save"]); assert.ok(h.state.error); assert.equal(h.state.busy, false);
    await h.navigation.choose("cancel");
    assert.deepEqual(h.calls.at(-1), ["commit", 1, false]);
  }
  const h = navigationHarness(); h.commit = async () => { throw new Error("Hide failed"); };
  h.navigation.receive(request(1)); await h.navigation.choose("discard");
  assert.equal(h.state.pending.sequence, 1); assert.equal(h.state.error, "Hide failed");
  h.commit = async () => {}; await h.navigation.choose("cancel"); assert.equal(h.state.pending, null);
});

test("launcher requests arriving during ordinary saves wait and recheck dirty state", async () => {
  for (const success of [true, false]) {
    const h = navigationHarness(); h.navigation.setSaving(true);
    h.navigation.receive(request(1, { kind: "navigate", route: { mode: "list" } }));
    assert.equal(h.state, null); assert.deepEqual(h.calls, []);
    h.dirty = !success; h.navigation.setSaving(false); await tick();
    if (success) assert.deepEqual(h.calls, [["commit", 1, true], ["accepted", 1]]);
    else { assert.equal(h.state.pending.sequence, 1); assert.deepEqual(h.calls, []); }
  }
});

test("delete sends exact ID, revision and confirmation; gates concurrent mutations and retries errors", async () => {
  let finish, fail; const calls = [];
  const service = createStudioService((command, args) => { calls.push([command, args]); return new Promise((resolve, reject) => { finish = resolve; fail = reject; }); });
  await assert.rejects(service.delete("id", 4, false), /Confirm/); assert.equal(calls.length, 0);
  const first = service.delete("id", 4, true);
  await assert.rejects(service.delete("id", 4, true), /already in progress/);
  await assert.rejects(service.save({}, 4), /already in progress/);
  fail({ code: "revision_conflict", message: "Reload before deleting" });
  await assert.rejects(first, { code: "revision_conflict" });
  const retry = service.delete("id", 5, true); finish(); await retry;
  assert.deepEqual(calls, [["delete_workflow", { id: "id", expectedRevision: 4, confirmed: true }], ["delete_workflow", { id: "id", expectedRevision: 5, confirmed: true }]]);
  const save = service.save({}, 5);
  await assert.rejects(service.delete("id", 5, true), /already in progress/); finish(); await save;
});

const action = { id: "volume", display_name: "Volume", parameters: [{ key: "percent", label: "Percent", type: "number", min: 0, max: 100, integer: true, required: true }], schema_version: 1, workflow_compatible: true, terminal: false, confirmation: "none" };
const view = { workflow: { id: "workflow-id", name: "Work", description: "Start the day", aliases: ["morning"], format_version: 1, revision: 7, created_at: 100, updated_at: 200,
  steps: [{ id: "first", action_id: "volume", action_schema_version: 1, order: 0, label: "My volume", parameters: { percent: 25 } },
    { id: "second", action_id: "future", action_schema_version: 8, order: 1, label: "Keep me", parameters: { future: { nested: ["data", 2] } } }] }, executable: false, issues: ["Future action"] };

test("renaming, parameters and reorder save the same workflow and step identities with expected revision", async () => {
  const draft = createDraft(view);
  draft.name = "Renamed"; draft.steps[0].parameters.percent = "30";
  draft.steps = reorderSteps(draft.steps, "second", -1);
  const { payload, errors } = validateDraft(draft, [action], view);
  assert.deepEqual(errors, {}); assert.equal(payload.id, view.workflow.id);
  assert.deepEqual(payload.steps.map(s => s.id), ["second", "first"]);
  assert.deepEqual(payload.steps[0].parameters, view.workflow.steps[1].parameters);
  assert.equal(payload.steps[1].parameters.percent, 30);
  const service = createStudioService(async (command, args) => {
    assert.equal(command, "save_workflow"); assert.equal(args.expectedRevision, 7);
    assert.equal(args.payload.id, view.workflow.id); return { ...view, workflow: { ...args.payload, revision: 8 } };
  });
  assert.equal((await service.save(payload, view.workflow.revision)).workflow.revision, 8);
  assert.equal(view.workflow.name, "Work"); assert.equal(view.workflow.steps[0].parameters.percent, 25);
});

test("unavailable steps and unsupported metadata survive unchanged; altered/new references fail", () => {
  const draft = createDraft(view);
  assert.deepEqual(validateDraft(draft, [action], view).errors, {});
  const unsupported = { ...action, parameters: [{ key: "value", type: "future_control" }] };
  assert.deepEqual(validateDraft(draft, [unsupported], view).errors, {});
  assert.ok(validateDraft(draft, [action]).errors.second);
  draft.steps[1].parameters.future = "changed";
  assert.ok(validateDraft(draft, [action], view).errors.second);
  draft.steps[1] = { ...draft.steps[1], action_id: "volume", action_schema_version: 1, parameters: { percent: 40 } };
  assert.deepEqual(validateDraft(draft, [action], view).errors, {});
  assert.equal(draft.steps[1].id, "second");
});

test("dirty fingerprint recognizes numeric reversions but preserves malformed input and other changes", () => {
  const draft = createDraft(view), clean = draftFingerprint(draft, [action]);
  draft.steps[0].parameters.percent = "30"; assert.notEqual(draftFingerprint(draft, [action]), clean);
  draft.steps[0].parameters.percent = "25"; assert.equal(draftFingerprint(draft, [action]), clean);
  draft.steps[0].parameters.percent = ""; assert.notEqual(draftFingerprint(draft, [action]), clean);
  draft.steps[0].parameters.percent = 25; draft.name += "X"; assert.notEqual(draftFingerprint(draft, [action]), clean);
});

test("local workflow search includes name, description and aliases, distinguishes no matches, and sorts without mutation", () => {
  const other = { ...view, workflow: { ...view.workflow, id: "other", name: "Evening", description: null, aliases: [], updated_at: 300 } };
  const items = [view, other];
  assert.deepEqual(filterWorkflows(items, "  MORNING day  "), [view]);
  assert.deepEqual(filterWorkflows(items, "even"), [other]);
  assert.deepEqual(filterWorkflows(items, "missing"), []);
  assert.deepEqual(filterWorkflows(items, " "), [other, view]);
  assert.deepEqual(items, [view, other]); assert.deepEqual(filterWorkflows([], ""), []);
});
