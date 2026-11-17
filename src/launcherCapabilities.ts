import type { CapabilityRegistration } from "./capabilityMetadata.ts";

/** These prefixes drive the parser as well as discovery. Literal spaces matter. */
export const searchModeRegistry = [
  { id: "search.apps", mode: "apps", prefix: "", kind: "search_mode", displayName: "Search installed applications", category: "Applications",
    description: "Search installed desktop and Store applications by name, then select a result to launch it.", syntax: "<application name>", aliases: [], examples: ["calculator", "visual"], discoverable: true, actionIds: ["app.launch"] },
  { id: "search.files", mode: "files", prefix: "f ", kind: "search_mode", displayName: "Search files and folders", category: "File and Folder Search",
    description: "Search the local Everything index by filename or path. Select a file to open its association or a folder to open Explorer.", syntax: "f <filename or path>", aliases: [], examples: ["f meeting notes", "f Documents"], discoverable: true,
    actionIds: ["file.open", "folder.open"], launcherConfirmation: "sensitive_target", prerequisites: ["Everything desktop client running with a ready index", "ES command-line tool (es.exe) via NEXUS_ES_PATH or PATH"],
    parameters: [{ name: "query", description: "Literal filename or path text; Everything operators are treated as text. A leading f and literal space select this mode." }] },
  { id: "search.workflows", mode: "workflows", prefix: "w ", kind: "search_mode", displayName: "Search and run workflows", category: "Workflows",
    description: "Browse saved workflows or search their names and aliases; select a result to run its saved ID.", syntax: "w [name or alias]", aliases: [], examples: ["w ", "w quick"], discoverable: true,
    launcherConfirmation: "per_step", prerequisites: ["A saved workflow for execution"], parameters: [{ name: "query", description: "Optional saved workflow name or alias; names are scoped to w mode." }] },
] as const satisfies readonly (CapabilityRegistration & { mode: "apps" | "files" | "workflows"; prefix: string })[];

/** Keyboard bindings are consumed by launcher event handling. */
export const launcherKeys = { next: "ArrowDown", previous: "ArrowUp", execute: "Enter", dismiss: "Escape" } as const;

export const launcherShortcutRegistry = [
  { id: "shortcut.navigate", kind: "shortcut", displayName: "Navigate results", category: "Getting Started", description: "Move through results while the search input is focused; selection wraps at the ends.", syntax: `${launcherKeys.previous} / ${launcherKeys.next}`, aliases: [], examples: ["ArrowDown"], discoverable: true },
  { id: "shortcut.execute", kind: "shortcut", displayName: "Execute selected result", category: "Getting Started", description: "Open or run the selected result. App and workflow rows execute on key release; repeated key-down does not duplicate execution.", syntax: launcherKeys.execute, aliases: [], examples: ["Enter"], discoverable: true },
  { id: "shortcut.reveal", kind: "shortcut", displayName: "Show selected file in folder", category: "File and Folder Search", description: "Reveal the selected file or folder in Explorer while in file search mode.", syntax: `Ctrl+${launcherKeys.execute}`, aliases: [], examples: ["Ctrl+Enter"], discoverable: true },
  { id: "shortcut.dismiss", kind: "shortcut", displayName: "Dismiss and clear launcher", category: "General NEXUS commands", description: "Clear the query, selection, transient results and pending launcher confirmation, then hide NEXUS. Active workflows continue.", syntax: launcherKeys.dismiss, aliases: [], examples: ["Escape"], discoverable: true },
  { id: "shortcut.focus", kind: "shortcut", displayName: "Focus controls", category: "Getting Started", description: "Move focus between buttons and fields. Enter or Space activates a focused button; confirmation dialogs initially focus Cancel.", syntax: "Tab / Shift+Tab", aliases: [], examples: ["Tab", "Shift+Tab"], discoverable: true },
  { id: "shortcut.close", kind: "shortcut", displayName: "Close window", category: "General NEXUS commands", description: "Windows closes Studio through its draft guard. Closing the launcher hides it during active runs; otherwise it quits after resolving Studio drafts.", syntax: "Alt+F4", aliases: [], examples: ["Alt+F4"], discoverable: true },
  { id: "shortcut.action_picker", kind: "shortcut", displayName: "Close action picker", category: "Workflows", description: "Close the action search picker in Workflow Studio.", syntax: "Escape (Studio action search)", aliases: [], examples: ["Escape"], discoverable: true },
] as const satisfies readonly CapabilityRegistration[];

/** Existing buttons, not additional typed command syntax or a second dispatcher. */
export const launcherControlRegistry = {
  refreshApps: { id: "launcher.refresh_apps", kind: "interaction", displayName: "Refresh apps", category: "Applications", description: "Rebuild the installed application catalog. A refresh failure retains the usable catalog.", syntax: "Click Refresh apps (ordinary search)", aliases: [], examples: ["Refresh apps"], discoverable: true },
  retryFiles: { id: "launcher.retry_files", kind: "interaction", displayName: "Retry search", category: "File and Folder Search", description: "Retry the current nonempty file search.", syntax: "Click Retry search (f mode)", aliases: [], examples: ["Retry search"], discoverable: true },
  refreshWorkflows: { id: "launcher.refresh_workflows", kind: "interaction", displayName: "Refresh workflows", category: "Workflows", description: "Reload saved workflow results from local storage.", syntax: "Click Refresh workflows (w mode)", aliases: [], examples: ["Refresh workflows"], discoverable: true },
  runs: { id: "launcher.workflow_runs", kind: "interaction", displayName: "Workflow runs", category: "Workflows", description: "Inspect session progress and failures, approve or deny sensitive steps, or cancel an active run. Cancellation cannot undo admitted actions.", syntax: "Click Workflow runs", aliases: [], examples: ["Workflow runs"], discoverable: true },
  more: { id: "launcher.more", kind: "interaction", displayName: "Show more", category: "Getting Started", description: "Show another batch of results; scrolling and keyboard navigation also reveal further results.", syntax: "Click Show more", aliases: [], examples: ["Show more"], discoverable: true },
  studioManage: { id: "studio.manage", kind: "interaction", displayName: "Manage saved workflows", category: "Workflows", description: "Use Studio to filter, edit, rename, change aliases, add/remove/reorder action steps, and save workflows. Unsaved edits use Save / Discard / Cancel.", syntax: "edit workflow > Saved workflows", aliases: [], examples: ["edit workflow"], discoverable: true },
  studioDelete: { id: "studio.delete", kind: "interaction", displayName: "Delete saved workflow", category: "Workflows", description: "Select Delete beside a saved workflow, review its name, then explicitly confirm Delete workflow. Cancellation preserves the workflow.", syntax: "Saved workflows > Delete > Delete workflow", aliases: [], examples: ["Delete workflow"], discoverable: true, launcherConfirmation: "always" },
  drag: { id: "launcher.move", kind: "interaction", displayName: "Move launcher", category: "General NEXUS commands", description: "Drag the NEXUS header to move the launcher within the desktop.", syntax: "Drag launcher header", aliases: [], examples: ["Drag launcher header"], discoverable: true },
  help: { id: "nexus.help", kind: "command", displayName: "Help", category: "General NEXUS commands", description: "Discover NEXUS capabilities through the read-only metadata catalog. The Help Center window is not implemented in Stage A.", syntax: "help", aliases: [], examples: ["help"], discoverable: true,
    availability: { status: "unavailable", reason: "Metadata only in Stage A; there is no help execution handler or Help Center UI yet." } },
} as const satisfies Record<string, CapabilityRegistration>;
