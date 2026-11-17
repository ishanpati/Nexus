import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { discoverCapabilities, registeredCapabilities } from "../src/commandDiscovery.ts";
import { groupCapabilities } from "../src/capabilityMetadata.ts";
import { commandRegistry, findCommands } from "../src/commands.ts";
import { parseSystemCommand, systemCommandRegistry } from "../src/systemCommands.ts";
import { searchMode } from "../src/fileSearch.ts";
import { launcherKeys, searchModeRegistry } from "../src/launcherCapabilities.ts";
import { createCommandDiscoveryService } from "../src/services/commandDiscoveryService.ts";

// Rust tests compare this contract fixture with the actual compiled registry.
const actions = JSON.parse(readFileSync(new URL("./fixtures/discovery-actions.json", import.meta.url), "utf8"));
const native = {
  actions,
  launcherShortcut: { keys: "Ctrl+Shift+Space", registered: true, warning: null },
  fileSearch: { status: "unavailable", reason: "ES missing" },
};

test("public IDs, descriptions, syntax and examples are complete and serializable", () => {
  const items = discoverCapabilities(native);
  assert.equal(new Set(items.map(item => item.id)).size, items.length);
  for (const item of items) {
    for (const field of ["id", "displayName", "category", "description", "syntax"]) assert.ok(item[field].trim(), item.id);
    assert.ok(item.examples.length, item.id);
    assert.equal(item.discoverable, true);
    for (const key of ["request", "execute", "executor", "validate", "prefix", "mode"]) assert.equal(key in item, false, `${item.id}: ${key}`);
  }
  assert.deepEqual(JSON.parse(JSON.stringify(items)).map(item => item.id), items.map(item => item.id));
  assert.throws(() => discoverCapabilities(native, [registeredCapabilities()[0], registeredCapabilities()[0]]), /Duplicate/);
  assert.throws(() => discoverCapabilities(native, [{ ...registeredCapabilities()[0], description: " " }]), /Incomplete/);
});

test("all command families, modes, workflow actions and existing controls are discovered", () => {
  const items = discoverCapabilities(native), ids = new Set(items.map(item => item.id));
  for (const command of Object.values(commandRegistry)) assert.ok(ids.has(`command.${command.id}`));
  for (const command of systemCommandRegistry) assert.ok(ids.has(`system.${command.id}`));
  for (const mode of searchModeRegistry) assert.ok(ids.has(mode.id));
  for (const action of actions) assert.ok(ids.has(`action.${action.id}`));
  for (const id of ["launcher.refresh_apps", "launcher.retry_files", "launcher.refresh_workflows", "launcher.workflow_runs", "studio.manage", "studio.delete", "shortcut.dismiss", "shortcut.reveal", "nexus.help"]) assert.ok(ids.has(id), id);
  assert.equal(ids.has("execute_command"), false);
  assert.equal(ids.has("clear_list"), false);
  assert.equal(findCommands("help").length, 0); // Stage A has no Help Center executor.
  assert.equal(items.find(item => item.id === "nexus.help").availability.status, "unavailable");
});

test("new central command registrations and categories appear without editing discovery", () => {
  commandRegistry.test_extension = {
    id: "test_extension", label: "Test extension", description: "A test-only registered capability.", aliases: ["extension"],
    request: { id: "test_extension", args: {} },
    discovery: { category: "Future category", examples: ["test extension"], discoverable: true },
  };
  commandRegistry.test_private = { ...commandRegistry.test_extension, id: "test_private", discovery: { ...commandRegistry.test_extension.discovery, discoverable: false } };
  try {
    const items = discoverCapabilities(native);
    assert.equal(groupCapabilities(items).get("Future category")[0].id, "command.test_extension");
    assert.ok(findCommands("test extension").some(command => command.id === "test_extension"));
    assert.equal(items.some(item => item.id === "command.test_private"), false);
  } finally {
    delete commandRegistry.test_extension;
    delete commandRegistry.test_private;
  }
});

test("all action references resolve and reuse exact authoritative schemas and policies", () => {
  const items = discoverCapabilities(native);
  for (const item of items) {
    assert.equal(item.workflowCompatible, item.actions.some(action => action.workflow_compatible));
    for (const action of item.actions) assert.equal(action, actions.find(candidate => candidate.id === action.id));
  }
  const lock = items.find(item => item.id === "system.lock");
  assert.equal(lock.launcherConfirmation, "none");
  assert.equal(lock.actions[0].confirmation, "always");
  assert.equal(lock.actions[0].terminal, true);
  assert.equal(items.find(item => item.id === "system.sleep").launcherConfirmation, "always");
  assert.equal(items.find(item => item.id === "system.audio_status").workflowCompatible, false);
  assert.throws(() => discoverCapabilities({ ...native, actions: actions.filter(action => action.id !== "audio.set_volume") }), /Unknown action metadata/);
});

test("typed command examples and aliases use the existing parser and matcher", () => {
  for (const command of systemCommandRegistry) {
    for (const example of command.discovery.examples) {
      const parsed = parseSystemCommand(example);
      assert.equal(parsed.status, "ready", example);
      assert.equal(parsed.commandId, command.id, example);
      const item = discoverCapabilities(native).find(item => item.id === `system.${command.id}`);
      if (command.id === "volume") {
        const parameter = item.actions.flatMap(action => action.parameters).find(parameter => parameter.key in parsed.action.args);
        const value = parsed.action.args[parameter.key];
        assert.ok(Number.isInteger(value) && value >= parameter.min && value <= parameter.max);
      }
      if (parsed.action.id === "open_settings") assert.ok(item.actions[0].parameters[0].options.includes(parsed.action.args.page));
    }
    for (const alias of command.names) assert.equal(parseSystemCommand(alias).commandId, command.id);
  }
  for (const command of Object.values(commandRegistry)) {
    for (const example of [...command.discovery.examples, ...command.aliases]) assert.ok(findCommands(example).some(match => match.id === command.id), example);
  }
});

test("search mode examples and literal prefixes retain exclusive parser semantics", () => {
  for (const mode of searchModeRegistry) {
    for (const example of mode.examples) assert.equal(searchMode(example).kind, mode.mode, example);
  }
  for (const input of ["f", "w", " f report", "w\tquick"]) assert.equal(searchMode(input).kind, "apps");
  assert.deepEqual(searchMode("F report"), { kind: "files", query: "report" });
  assert.deepEqual(searchMode("W quick"), { kind: "workflows", query: "quick" });
});

test("shortcut config and unavailable dependencies remain visible without execution", async () => {
  const calls = [];
  const list = createCommandDiscoveryService(async command => {
    calls.push(command);
    return { ...native, launcherShortcut: { keys: "Alt+Space", registered: false, warning: "Already registered" } };
  });
  const items = await list();
  assert.deepEqual(calls, ["command_discovery_metadata"]);
  const shortcut = items.find(item => item.id === "shortcut.launcher");
  assert.equal(shortcut.syntax, "Alt+Space");
  assert.equal(shortcut.availability.status, "unavailable");
  assert.equal(shortcut.availability.reason, "Already registered");
  const files = items.find(item => item.id === "search.files");
  assert.equal(files.availability, native.fileSearch);
  assert.ok(files.prerequisites.some(text => text.includes("Everything")));
  assert.equal(items.find(item => item.id === "shortcut.execute").syntax, launcherKeys.execute);
  assert.equal(items.find(item => item.id === "shortcut.dismiss").syntax, launcherKeys.dismiss);
  await assert.rejects(createCommandDiscoveryService(async () => { throw new Error("IPC unavailable"); })(), /IPC unavailable/);
});
