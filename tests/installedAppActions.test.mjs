import assert from "node:assert/strict";
import { test } from "node:test";
import { createInstalledAppActions, createIconLoader, validIconPixels } from "../src/installedAppActions.ts";
import { recordSuccessfulLaunch } from "../src/launchUsage.ts";

test("launch and icon IPC send only the selected catalog ID", async () => {
  const calls = [];
  const actions = createInstalledAppActions(async (...args) => { calls.push(args); return { launched: true, warning: null }; });
  await actions.launch("shortcut:catalog-owned-link");
  await actions.icon("packaged:registered-id");
  assert.deepEqual(calls, [
    ["launch_installed_app", { appId: "shortcut:catalog-owned-link" }],
    ["get_installed_app_icon", { appId: "packaged:registered-id" }],
  ]);
});

test("launch errors propagate, while hide warnings retain successful-launch credit", async () => {
  const failing = createInstalledAppActions(async () => { throw "Application no longer installed"; });
  await assert.rejects(failing.launch("missing"), (error) => error === "Application no longer installed");
  const successful = createInstalledAppActions(async () => ({ launched: true, warning: "Could not hide NEXUS" }));
  const result = await successful.launch("known");
  assert.equal(result.launched, true);
  assert.equal(result.warning, "Could not hide NEXUS");
  assert.deepEqual(recordSuccessfulLaunch({}, "app:known"), { "app:known": 1 });
});

test("icon loader caches successful and failed requests across repeated searches", async () => {
  const calls = [];
  const load = createIconLoader(async (id) => { calls.push(id); if (id === "bad") throw new Error("missing icon"); return "local-image"; });
  const first = load("good");
  assert.equal(first, load("good"));
  assert.equal(await first, "local-image");
  assert.equal(await load("good"), "local-image");
  assert.equal(await load("bad"), null);
  assert.equal(await load("bad"), null);
  assert.deepEqual(calls, ["good", "bad"]);
});

test("icon loader limits outstanding extractions and lets all queued work finish", async () => {
  let running = 0;
  let max = 0;
  const load = createIconLoader(async (id) => {
    running++; max = Math.max(max, running);
    await new Promise((resolve) => setTimeout(resolve, 1));
    running--; return id;
  });
  const results = await Promise.all(Array.from({ length: 12 }, (_, i) => load(String(i))));
  assert.equal(results.length, 12);
  assert.equal(max, 2);
});

test("icon pixel validation rejects invalid dimensions, lengths and channels", () => {
  assert.equal(validIconPixels({ width: 1, height: 1, rgba: [1, 2, 3, 255] }), true);
  for (const icon of [
    { width: 0, height: 1, rgba: [] }, { width: 300, height: 1, rgba: [] },
    { width: 1, height: 1, rgba: [1, 2, 3] }, { width: 1, height: 1, rgba: [1, 2, 3, -1] },
  ]) assert.equal(validIconPixels(icon), false);
});
