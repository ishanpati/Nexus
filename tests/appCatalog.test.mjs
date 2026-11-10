import assert from "node:assert/strict";
import { test } from "node:test";
import { createCatalogStore } from "../src/appCatalog.ts";
import { indexApplications, rankApplications } from "../src/appSearch.ts";

const catalog = { apps: [], generation: 1, warnings: [], sources: [] };
const deferred = () => { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; };

test("startup requests coalesce; typing and subsequent loads do not call discovery", async () => {
  const response = deferred();
  let calls = 0;
  const store = createCatalogStore(() => { calls++; return response.promise; });
  const first = store.load();
  assert.equal(store.load(), first);
  assert.equal(store.getSnapshot().loading, true);
  response.resolve(catalog);
  await first;
  const snapshot = store.getSnapshot();
  for (const query of ["a", "ap", "app", "nonsense"]) rankApplications(indexApplications(snapshot.catalog.apps), query);
  await store.load();
  assert.equal(calls, 1);
  assert.equal(store.getSnapshot(), snapshot);
  assert.equal(snapshot.loading, false);
});

test("refresh keeps old data responsive and replaces it only on success", async () => {
  const response = deferred();
  const args = [];
  const store = createCatalogStore((refresh) => { args.push(refresh); return refresh ? response.promise : Promise.resolve(catalog); });
  await store.load();
  const pending = store.load(true);
  assert.equal(store.getSnapshot().catalog, catalog);
  assert.equal(store.getSnapshot().loading, true);
  assert.equal(store.load(true), pending);
  const replacement = { ...catalog, generation: 2 };
  response.resolve(replacement);
  await pending;
  assert.equal(store.getSnapshot().catalog, replacement);
  assert.deepEqual(args, [false, true]);
});

test("refresh error retains old catalog and retry recovers; listeners can unsubscribe", async () => {
  let fail = false;
  const store = createCatalogStore(async () => { if (fail) throw new Error("access denied"); return catalog; });
  let updates = 0;
  const unsubscribe = store.subscribe(() => updates++);
  await store.load();
  fail = true;
  await store.load(true);
  assert.equal(store.getSnapshot().catalog, catalog);
  assert.match(store.getSnapshot().error, /access denied/);
  assert.equal(store.getSnapshot().loading, false);
  unsubscribe();
  const previous = updates;
  fail = false;
  await store.load(true);
  assert.equal(updates, previous);
  assert.equal(store.getSnapshot().error, "");
});

test("initial errors are visible and allow retry; partial catalogs retain diagnostics", async () => {
  let fail = true;
  const partial = { ...catalog, warnings: ["unavailable folder"], sources: [{ completed: false }] };
  const store = createCatalogStore(async () => { if (fail) throw "Windows unavailable"; return partial; });
  await store.load();
  assert.equal(store.getSnapshot().catalog, null);
  assert.match(store.getSnapshot().error, /Windows unavailable/);
  fail = false;
  await store.load(true);
  assert.equal(store.getSnapshot().catalog, partial);
});
