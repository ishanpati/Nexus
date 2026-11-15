import assert from "node:assert/strict";
import { test } from "node:test";
import { createSystemActions, confirmationGate, systemActionError } from "../src/services/systemActionService.ts";
import { parseSystemCommand, suggestSystemCommands } from "../src/systemCommands.ts";

test("system suggestions never execute; explicit activation dispatches fixed actions", async () => {
  const calls = [];
  const accepted = { status: "accepted", message: "Accepted", warning: null };
  const service = createSystemActions(async (...args) => { calls.push(args); return accepted; });
  for (const text of ["sound settings", "wifi settings", "bluetooth settings", "display settings", "lock", "sleep"]) {
    suggestSystemCommands(text);
    assert.equal(calls.length, 0);
    assert.equal(parseSystemCommand(text).status, "ready");
  }
  for (const page of ["sound", "wifi", "bluetooth", "display"]) {
    const request = parseSystemCommand(page + " settings").action;
    assert.deepEqual(await service.execute(request), accepted);
    assert.deepEqual(calls.at(-1), ["execute_system_action", { request: { id: "open_settings", args: { page } }, confirmationToken: null }]);
  }
  await service.execute(parseSystemCommand("lock").action);
  assert.equal(calls.at(-1)[1].request.id, "lock"); // Mock transport only.
});

test("unknown destinations, extra arguments and non-system actions never reach transport", async () => {
  const service = createSystemActions(async () => assert.fail("unexpected native call"));
  for (const request of [
    { id: "open_settings", args: { page: "ms-settings:privacy" } },
    { id: "open_settings", args: { page: "sound", uri: "anything" } },
    { id: "lock", args: { force: true } }, { id: "shutdown", args: {} },
    { id: "set_volume", args: { percent: 35 } },
  ]) await assert.rejects(service.execute(request), /Invalid Windows system action/);
  await assert.rejects(service.execute({ id: "lock", args: {} }, "ticket"), /Invalid Windows system action/);
});

test("sleep preparation does not auto-confirm; confirmation and cancellation are separate IPC calls", async () => {
  const calls = [];
  const prompt = { status: "confirmation_required", token: "ticket", message: "Sleep now?" };
  const service = createSystemActions(async (...args) => { calls.push(args); return prompt; });
  const request = { id: "sleep", args: {} };
  assert.deepEqual(await service.execute(request), prompt);
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1].confirmationToken, null);
  await service.cancel(prompt.token);
  assert.deepEqual(calls[1], ["cancel_system_confirmation", { token: "ticket" }]);
  await service.execute(request, prompt.token);
  assert.equal(calls[2][1].confirmationToken, "ticket");
});

test("dialog callbacks share a one-use gate, including double activation and cancellation", () => {
  for (const first of ["confirm", "cancel"]) {
    const gate = confirmationGate();
    const calls = [];
    assert.equal(gate(() => calls.push(first)), true);
    assert.equal(gate(() => calls.push("confirm")), false);
    assert.equal(gate(() => calls.push("cancel")), false);
    assert.deepEqual(calls, [first]);
  }
});

test("structured failures propagate without retry and accepted warnings remain success", async () => {
  const failure = { code: "restricted", message: "Windows policy restricts sleep." };
  let calls = 0;
  const service = createSystemActions(async () => { calls++; throw failure; });
  await assert.rejects(service.execute({ id: "sleep", args: {} }, "ticket"), error => error === failure);
  assert.equal(calls, 1);
  assert.equal(systemActionError(failure), failure.message);
  assert.match(systemActionError("offline"), /offline/);
  const accepted = { status: "accepted", message: "Accepted", warning: "Could not hide; do not repeat." };
  assert.deepEqual(await createSystemActions(async () => accepted).execute({ id: "lock", args: {} }), accepted);
});
