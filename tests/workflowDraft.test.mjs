import assert from "node:assert/strict";
import { test } from "node:test";
import { createDraft, descriptorProblem, newStep, parameterValue, removeStep, reorderSteps, stepSummary, validateDraft } from "../src/workflowDraft.ts";
import { createStudioService } from "../src/services/workflowStudioService.ts";

const action = (id, parameters = [], extra = {}) => ({ id, display_name: id, description: "Test action", category: "Test",
  icon: "application", parameters, schema_version: 1, workflow_compatible: true, confirmation: "none", terminal: false, ...extra });
const parameter = (type, extra = {}) => ({ key: "value", label: "Value", type, required: true, ...extra });
const catalog = [action("app.launch", [parameter("application", { key: "application_id" })]),
  action("folder.open", [parameter("folder", { key: "path" })]), action("audio.set_volume", [parameter("number", { key: "percent", min: 0, max: 100, integer: true })])];
function threeSteps() {
  const draft = createDraft(); draft.name = "Workday";
  draft.steps = catalog.map((d, i) => newStep(d, i));
  draft.steps[0].parameters.application_id = "catalog:app-id";
  draft.steps[1].parameters.path = "C:\\資料 space\\Work folder";
  draft.steps[2].parameters.percent = "25";
  return draft;
}
test("three actions normalize to a typed payload; save/reload keeps identities and parameters", async () => {
  let saved;
  const calls = [];
  const service = createStudioService(async (command, args) => {
    calls.push(command);
    if (command === "save_workflow") saved = { workflow: { ...structuredClone(args.payload), revision: (args.expectedRevision ?? 0) + 1 }, issues: [], executable: true };
    return structuredClone(saved);
  });
  const draft = threeSteps(); draft.description = "My workspace"; draft.aliases = "day\nworkspace";
  const firstId = draft.steps[0].id, volumeId = draft.steps[2].id;
  draft.steps = reorderSteps(draft.steps, volumeId, -1);
  assert.equal(draft.steps[1].id, volumeId);
  assert.equal(draft.steps[1].parameters.percent, "25");
  const { errors, payload } = validateDraft(draft, catalog);
  assert.deepEqual(errors, {}); assert.equal(payload.steps[1].parameters.percent, 25);
  assert.deepEqual(payload.steps.map(s => s.order), [0, 1, 2]);
  const result = await service.save(payload, null);
  const loaded = createDraft(await service.get(result.workflow.id));
  assert.deepEqual(loaded.steps, payload.steps);
  assert.equal(loaded.steps[0].id, firstId);
  assert.equal(loaded.steps[2].parameters.path, "C:\\資料 space\\Work folder");
  loaded.steps[1].parameters.percent = "30";
  loaded.steps = removeStep(loaded.steps, firstId);
  assert.equal(loaded.steps[0].id, volumeId); assert.equal(loaded.steps[0].parameters.percent, "30");
  assert.deepEqual(loaded.steps.map(s => s.order), [0, 1]);
  assert.deepEqual(calls, ["save_workflow", "get_workflow"]);
});
test("invalid name, empty steps, descriptions and duplicate aliases fail inline", () => {
  const draft = createDraft();
  assert.ok(validateDraft(draft, catalog).errors.name);
  assert.ok(validateDraft(draft, catalog).errors.steps);
  draft.name = "Work"; draft.aliases = "work"; draft.description = "字".repeat(1400);
  const { errors } = validateDraft(draft, catalog);
  assert.ok(errors.aliases); assert.ok(errors.description);
});
test("blank and malformed numbers never coerce to zero; ranges and safe integers are enforced", () => {
  const p = parameter("number", { min: 0, max: 100, integer: true });
  for (const raw of ["", " ", "25cats", "0x19", "1e1", "NaN", "Infinity", "25.5", "-1", "101", null, false, NaN, Infinity]) {
    assert.ok(parameterValue(p, raw).error, `Rejected ${String(raw)}`);
  }
  assert.equal(parameterValue(p, "0").value, 0); assert.equal(parameterValue(p, "25").value, 25);
  assert.ok(parameterValue({ ...p, max: 1e20 }, "9999999999999999999").error);
});
test("optional omissions, false, choice restrictions and defaults follow metadata", () => {
  const d = action("test.new_action", [parameter("text", { max_length: 20 }),
    parameter("number", { key: "count", min: 1, max: 10, integer: true, required: false, default: 3 }),
    parameter("boolean", { key: "enabled" }), parameter("choice", { key: "mode", options: ["one", "two"] }),
    parameter("folder", { key: "optional_folder", required: false })]);
  assert.equal(descriptorProblem(d), "");
  const draft = createDraft(); draft.name = "Dynamic"; draft.steps = [newStep(d, 0)];
  draft.steps[0].parameters = { value: "Unicode 資料", count: "", enabled: false, mode: "two", optional_folder: "" };
  const result = validateDraft(draft, [d]); assert.deepEqual(result.errors, {});
  assert.deepEqual(result.payload.steps[0].parameters, { value: "Unicode 資料", count: 3, enabled: false, mode: "two" });
  draft.steps[0].parameters.mode = "three";
  assert.ok(validateDraft(draft, [d]).errors[`${draft.steps[0].id}.mode`]);
});
test("unknown actions, parameter types, saved versions and extra parameters cannot be silently saved", () => {
  const unknown = action("future.action", [parameter("future_control")]);
  assert.match(descriptorProblem(unknown), /Unsupported parameter type/);
  const draft = threeSteps();
  draft.steps[0].action_id = "removed.action";
  assert.match(validateDraft(draft, catalog).errors[draft.steps[0].id], /unavailable/);
  draft.steps[0].action_id = catalog[0].id; draft.steps[0].action_schema_version = 999;
  assert.match(validateDraft(draft, catalog).errors[draft.steps[0].id], /schema version/);
  draft.steps[0].action_schema_version = 1; draft.steps[0].parameters.confirmed = true;
  assert.match(validateDraft(draft, catalog).errors[draft.steps[0].id], /Unknown parameter/);
  const clone = structuredClone(draft.steps);
  assert.equal(stepSummary(clone[0], unknown, []), "Unavailable action — parameters preserved.");
  assert.deepEqual(draft.steps, clone);
});
test("terminal placement is driven by registry metadata for any ID", () => {
  const terminal = action("test.final", [], { terminal: true, confirmation: "always" });
  const draft = threeSteps(); draft.steps.unshift(newStep(terminal, 0));
  assert.match(validateDraft(draft, [...catalog, terminal]).errors[draft.steps[0].id], /final step/);
  draft.steps.push(draft.steps.shift());
  assert.deepEqual(validateDraft(draft, [...catalog, terminal]).errors, {});
});
test("duplicate saves share a synchronous guard and failures allow a corrected retry", async () => {
  let resolve, reject, count = 0;
  const service = createStudioService(() => { count++; return new Promise((yes, no) => { resolve = yes; reject = no; }); });
  const payload = validateDraft(threeSteps(), catalog).payload;
  const first = service.save(payload, null);
  await assert.rejects(service.save(payload, null), /already in progress/); assert.equal(count, 1);
  reject({ code: "duplicate_name", message: "Name already exists" });
  await assert.rejects(first, { code: "duplicate_name" });
  const second = service.save(payload, null); resolve({ workflow: payload }); await second;
  assert.equal(count, 2);
});
test("picker cancellation and Unicode paths are passed through without shell conversion", async () => {
  const calls = [], values = [null, "C:\\資料 space\\my file.txt"];
  const service = createStudioService(async (command, args) => { calls.push([command, args]); return values.shift(); });
  assert.equal(await service.pickPath("folder"), null);
  assert.equal(await service.pickPath("file"), "C:\\資料 space\\my file.txt");
  assert.deepEqual(calls, [["pick_workflow_path", { kind: "folder" }], ["pick_workflow_path", { kind: "file" }]]);
});
