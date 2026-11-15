import assert from "node:assert/strict";
import { test } from "node:test";
import { createAudioActions, isAudioAction, audioStateMessage, audioErrorMessage } from "../src/services/audioService.ts";
import { parseSystemCommand, suggestSystemCommands } from "../src/systemCommands.ts";

test("typing and suggesting makes no audio calls; activation sends one closed request", async () => {
  const calls = [];
  const state = { volume_percent: 35, muted: false, endpoint_id: "test" };
  const execute = createAudioActions(async (...args) => { calls.push(args); return state; });
  for (const text of ["volume 35", "volume up", "volume down", "mute", "unmute", "volume status", "audio status", "mute status"]) {
    suggestSystemCommands(text); assert.equal(calls.length, 0);
    assert.equal(isAudioAction(parseSystemCommand(text).action), true);
  }
  assert.deepEqual(await execute(parseSystemCommand("volume 35").action), state);
  assert.deepEqual(calls, [["execute_audio", { request: { id: "set_volume", args: { percent: 35 } } }]]);
  assert.equal(audioStateMessage(state), "Volume 35% · Unmuted");
  assert.match(audioStateMessage({ ...state, muted: true }), /Muted/);
});

test("non-audio and invalid requests cannot enter the audio transport", async () => {
  const execute = createAudioActions(async () => { throw new Error("must not invoke"); });
  for (const request of [parseSystemCommand("sleep").action, parseSystemCommand("sound settings").action,
    { id: "set_volume", args: { percent: NaN } }, { id: "set_volume", args: { percent: 101 } }]) {
    await assert.rejects(execute(request), /Invalid audio command/);
  }
});

test("structured failures remain errors and warn against duplicate relative adjustments", async () => {
  const failure = { code: "device_changed", message: "Output changed.", may_have_changed: true, hresult: null };
  const execute = createAudioActions(async () => { throw failure; });
  await assert.rejects(execute(parseSystemCommand("volume +10").action), (error) => error === failure);
  assert.match(audioErrorMessage(failure), /audio status before retrying/);
  assert.equal(audioErrorMessage({ ...failure, may_have_changed: false }), "Output changed.");
});
