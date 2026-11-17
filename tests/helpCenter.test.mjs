import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync, existsSync } from "node:fs";
import { registerHooks } from "node:module";
import { filterHelp, helpCategories, describeParameter, copyHelpExample } from "../src/helpCenter.ts";
import { discoverCapabilities } from "../src/commandDiscovery.ts";
import { commandRegistry } from "../src/commands.ts";
import { searchCommandsAndApps } from "../src/commandSearch.ts";
import { dispatchResult, executeWithLifecycle } from "../src/resultExecution.ts";
import { createCommandDiscoverySnapshotService } from "../src/services/commandDiscoveryService.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const native = {
  applicationVersion: "9.8.7", actions: JSON.parse(readFileSync(new URL("./fixtures/discovery-actions.json", import.meta.url))),
  launcherShortcut: { keys: "Ctrl+Shift+Space", registered: true, warning: null },
  fileSearch: { status: "unavailable", reason: "ES missing" },
};
const catalog = discoverCapabilities(native);

test("Help and partial hel selections dispatch only the selected ID and clear/hide after opening", async () => {
  for (const query of ["help", "hel"]) {
    const row = searchCommandsAndApps(query, [])[0];
    assert.equal(row.command.id, "help");
    for (const fail of [true, false]) {
      const calls = [];
      await executeWithLifecycle(() => dispatchResult(row, { command: async id => {
        calls.push(`open:${id}`); if (fail) throw new Error("Help failed");
      } }), { reset: () => calls.push("clear"), hide: async () => calls.push("hide"), restore: () => calls.push("restore"), error: message => calls.push(message) });
      assert.deepEqual(calls, fail ? ["open:help", "Help failed"] : ["open:help", "clear", "hide"]);
    }
  }
});

test("help search covers name, syntax, aliases, descriptions, examples and categories", () => {
  for (const [query, id] of [["audio status", "system.audio_status"], ["signed delta", "system.volume"], ["wireless settings", "system.wifi_settings"],
    ["percentage points", "action.audio.adjust_volume"], ["meeting notes", "search.files"], ["Power/System Controls", "system.lock"]]) {
    assert.ok(filterHelp(catalog, query).some(item => item.id === id), query);
    assert.deepEqual(filterHelp(catalog, query.toUpperCase()).map(item => item.id), filterHelp(catalog, query).map(item => item.id));
  }
  const distractor = { ...catalog[0], id: "distractor", displayName: "Unrelated", syntax: "other", aliases: [], description: "Learn about help", examples: [] };
  assert.equal(filterHelp([distractor, ...catalog], "help")[0].id, "nexus.help");
});

test("category and text filters intersect, hidden entries stay out, empty filters restore the catalog", () => {
  assert.deepEqual(filterHelp(catalog, "volume", "Windows Settings"), []);
  assert.ok(filterHelp(catalog, "volume", "Audio").length);
  assert.equal(filterHelp(catalog, "").length, catalog.length);
  assert.equal(filterHelp(catalog, "impossible_nonexistent_query").length, 0);
  assert.deepEqual(filterHelp([{ ...catalog[0], discoverable: false }], ""), []);
  assert.deepEqual(helpCategories([]), []);
  assert.equal(helpCategories(catalog)[0][0], "Getting Started");
});

test("new command and unknown category automatically reach the same help view model", () => {
  commandRegistry.test_help_extension = { id: "test_help_extension", label: "Future command", description: "A temporary test capability.", aliases: ["future alias"], request: { id: "test_help_extension", args: {} },
    discovery: { category: "A future category", examples: ["future command"], discoverable: true } };
  try {
    const items = discoverCapabilities(native);
    assert.ok(helpCategories(items).some(([name]) => name === "A future category"));
    assert.equal(filterHelp(items, "future alias", "A future category")[0].id, "command.test_help_extension");
  } finally { delete commandRegistry.test_help_extension; }
});

test("parameter presentation uses the supplied schema and handles optional/defaulted fields", () => {
  assert.equal(describeParameter({ type: "number", min: -2, max: 8, integer: true, required: true }), "Whole number from -2 to 8. Required.");
  assert.match(describeParameter({ type: "choice", options: ["one", "two"], required: false, default: "one" }), /Choose one, two. Optional. Default: one/);
  assert.match(describeParameter({ type: "text", max_length: 25, required: false }), /25 characters/);
  for (const type of ["boolean", "application", "file", "folder", "future_type"]) assert.ok(describeParameter({ type, required: false }).length);
});

test("copy writes the exact example including whitespace and reports recoverable errors", async () => {
  const writes = [];
  for (const example of ["w ", "volume -10", "f meeting notes", "<example & text>"]) {
    assert.equal(await copyHelpExample(example, async text => { writes.push(text); }), "Copied example.");
    assert.equal(writes.at(-1), example);
  }
  assert.match(await copyHelpExample("help", async () => { throw new Error("Denied"); }), /Could not copy/);
});

test("metadata snapshot gets authoritative version and performs only the read-only discovery request", async () => {
  const calls = [];
  const snapshot = await createCommandDiscoverySnapshotService(async command => { calls.push(command); return native; })();
  assert.deepEqual(calls, ["command_discovery_metadata"]);
  assert.equal(snapshot.version, "9.8.7");
  assert.equal(snapshot.items.length, catalog.length);
});

test("actual Help command service uses the dedicated no-argument open request", async () => {
  const calls = [];
  globalThis.__helpTestInvoke = async (...args) => { calls.push(args); };
  const hook = registerHooks({ resolve(specifier, context, nextResolve) {
    if (specifier === "@tauri-apps/api/core") return { url: "data:text/javascript,export const invoke = (...args) => globalThis.__helpTestInvoke(...args)", shortCircuit: true };
    if (specifier === "../commands") return nextResolve("../commands.ts", context);
    return nextResolve(specifier, context);
  } });
  try {
    const { executeCommand } = await import("../src/services/commandService.ts");
    await executeCommand("help");
    assert.deepEqual(calls, [["open_help_center"]]);
  } finally { hook.deregister(); delete globalThis.__helpTestInvoke; }
});

test("generic React cards and category navigation render current and future metadata safely", async () => {
  const hook = registerHooks({
    resolve(specifier, context, nextResolve) {
      if (specifier.endsWith(".css")) return { url: "data:text/javascript,export default {}", shortCircuit: true };
      if (specifier.startsWith(".") && !/\.[a-z]+$/i.test(specifier)) {
        for (const extension of [".ts", ".tsx"]) {
          if (existsSync(new URL(specifier + extension, context.parentURL))) return nextResolve(specifier + extension, context);
        }
      }
      return nextResolve(specifier, context);
    },
    load(url, context, nextLoad) {
      if (url.endsWith(".tsx")) return { format: "module", source: ts.transpileModule(readFileSync(new URL(url), "utf8"), { compilerOptions: { jsx: ts.JsxEmit.ReactJSX, module: ts.ModuleKind.ESNext } }).outputText, shortCircuit: true };
      return nextLoad(url, context);
    },
  });
  try {
    const { HelpCard, HelpNavigation } = await import("../src/components/HelpCenter.tsx");
    let copies = 0;
    const html = renderToStaticMarkup(createElement("div", null, catalog.map(item => createElement(HelpCard, { key: item.id, item, onCopy: () => copies++ }))));
    assert.equal((html.match(/<article /g) ?? []).length, catalog.length);
    assert.ok(html.includes("Whole number from 0 to 100"));
    assert.ok(html.includes("Must be the final step"));
    assert.equal(copies, 0);
    const future = { ...catalog[0], id: "future", displayName: "<script>future</script>", description: "<b>Text only</b>", category: "Future <category>", aliases: undefined, examples: undefined, actions: undefined, parameters: undefined, prerequisites: undefined, availability: undefined };
    const card = renderToStaticMarkup(createElement(HelpCard, { item: future, onCopy: () => copies++ }));
    assert.ok(card.includes("&lt;script&gt;future&lt;/script&gt;"));
    assert.equal(card.includes("<script>"), false);
    const nav = renderToStaticMarkup(createElement(HelpNavigation, { categories: helpCategories([...catalog, future]), count: catalog.length + 1, selected: future.category, onSelect: () => assert.fail("rendering must not select or execute") }));
    assert.ok(nav.includes("Future &lt;category&gt;"));
    assert.ok(nav.includes('aria-pressed="true"'));
  } finally { hook.deregister(); }
});
