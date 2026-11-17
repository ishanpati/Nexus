import type { CapabilityRegistration, DiscoverableCapability, NativeDiscoveryMetadata } from "./capabilityMetadata.ts";
import { commandRegistry } from "./commands.ts";
import { launcherControlRegistry, launcherShortcutRegistry, searchModeRegistry } from "./launcherCapabilities.ts";
import { requiresSystemConfirmation, systemCommandRegistry } from "./systemCommands.ts";

/** Adapters enumerate actual registrations, never a separate inventory of help rows. */
export function registeredCapabilities(): readonly CapabilityRegistration[] {
  return [
    ...Object.values(commandRegistry).map(command => ({
      id: `command.${command.id}`, kind: "command" as const, displayName: command.label,
      description: command.description, syntax: command.label.toLowerCase(), aliases: command.aliases,
      ...command.discovery,
    })),
    ...systemCommandRegistry.map(command => ({
      id: `system.${command.id}`, kind: "command" as const, displayName: command.label,
      description: command.help, aliases: command.names.slice(1), ...command.discovery,
      syntax: "syntax" in command.discovery ? command.discovery.syntax : command.names[0],
      launcherConfirmation: "action" in command && requiresSystemConfirmation(command.action) ? "always" as const : "none" as const,
      prerequisites: [command.discovery.category === "Audio" ? "Windows default multimedia audio output" : "Windows session; native support is checked on execution"],
    })),
    ...searchModeRegistry, ...launcherShortcutRegistry, ...Object.values(launcherControlRegistry),
  ];
}

/** Inject registrations for extension tests; production always enumerates the central registries. */
export function discoverCapabilities(native: NativeDiscoveryMetadata, registrations = registeredCapabilities()): readonly DiscoverableCapability[] {
  const actions = new Map(native.actions.map(action => [action.id, action]));
  if (actions.size !== native.actions.length) throw new Error("Duplicate action metadata ID.");
  const entries: readonly CapabilityRegistration[] = [
    ...registrations,
    ...native.actions.map(action => ({
      id: `action.${action.id}`, kind: "action" as const, displayName: action.display_name,
      description: action.description, category: action.category, syntax: `Workflow Studio > Add action > ${action.display_name}`,
      aliases: [], examples: [`Workflow Studio > ${action.display_name}`], discoverable: true, actionIds: [action.id],
    })),
    { id: "shortcut.launcher", kind: "shortcut", displayName: "Show or hide NEXUS", category: "Getting Started",
      description: "Toggle the launcher globally. Summoning starts with a cleared, focused input. Launching NEXUS again also reopens its existing instance.",
      syntax: native.launcherShortcut.keys, aliases: [], examples: [native.launcherShortcut.keys], discoverable: true,
      availability: { status: native.launcherShortcut.registered ? "available" : "unavailable", reason: native.launcherShortcut.warning ?? "Global shortcut registered." } },
  ];
  const ids = new Set<string>();
  return entries.filter(entry => entry.discoverable).map(entry => {
    if (!entry.id || ids.has(entry.id)) throw new Error(`Duplicate or empty capability ID: ${entry.id}`);
    ids.add(entry.id);
    if (![entry.displayName, entry.description, entry.category, entry.syntax].every(text => text.trim())) {
      throw new Error(`Incomplete discovery metadata: ${entry.id}`);
    }
    const associated = (entry.actionIds ?? []).map(id => {
      const action = actions.get(id);
      if (!action) throw new Error(`Unknown action metadata: ${id} (${entry.id})`);
      return action;
    });
    // Explicit projection prevents registry requests, prefixes and future private fields leaking out.
    return {
      id: entry.id, kind: entry.kind, displayName: entry.displayName, description: entry.description,
      category: entry.category, syntax: entry.syntax, aliases: entry.aliases, examples: entry.examples,
      discoverable: true, actionIds: entry.actionIds ?? [], actions: associated,
      workflowCompatible: associated.some(action => action.workflow_compatible),
      parameters: entry.parameters ?? [], prerequisites: entry.prerequisites ?? [],
      launcherConfirmation: entry.launcherConfirmation,
      availability: entry.availability ?? (entry.id === "search.files" || entry.id === "launcher.retry_files" ? native.fileSearch
        : { status: "unknown" as const, reason: "Implemented; current targets, device state and permissions are checked when used." }),
    };
  });
}
