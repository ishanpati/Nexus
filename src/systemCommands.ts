import { normalizeSearch } from "./appSearch.ts";

export interface SystemArguments {
  get_audio_state: Record<string, never>;
  set_volume: { readonly percent: number };
  adjust_volume: { readonly delta: number };
  set_mute: { readonly muted: boolean };
  open_settings: { readonly page: "sound" | "wifi" | "bluetooth" | "display" };
  lock: Record<string, never>;
  sleep: Record<string, never>;
}
export type SystemAction = {
  [Id in keyof SystemArguments]: { readonly id: Id; readonly args: SystemArguments[Id] }
}[keyof SystemArguments];

interface Definition {
  readonly id: string;
  readonly label: string;
  readonly names: readonly string[];
  readonly help: string;
  readonly action?: SystemAction;
}

export const systemCommandRegistry = [
  { id: "audio_status", label: "Audio status", names: ["volume status", "vol status", "audio status", "mute status"], help: "Read the current volume and mute state.", action: { id: "get_audio_state", args: {} } },
  { id: "volume", label: "Volume", names: ["volume", "vol"], help: "Use volume 0–100, volume +10/-10, or volume up/down." },
  { id: "mute", label: "Mute audio", names: ["mute", "mute audio"], help: "Mute the default audio output.", action: { id: "set_mute", args: { muted: true } } },
  { id: "unmute", label: "Unmute audio", names: ["unmute", "unmute audio"], help: "Unmute the default audio output.", action: { id: "set_mute", args: { muted: false } } },
  { id: "sound_settings", label: "Sound settings", names: ["sound settings", "audio settings"], help: "Open Windows sound settings.", action: { id: "open_settings", args: { page: "sound" } } },
  { id: "wifi_settings", label: "Wi-Fi settings", names: ["wifi settings", "wi-fi settings", "wireless settings"], help: "Open Windows Wi-Fi settings.", action: { id: "open_settings", args: { page: "wifi" } } },
  { id: "bluetooth_settings", label: "Bluetooth settings", names: ["bluetooth settings", "bt settings"], help: "Open Windows Bluetooth settings.", action: { id: "open_settings", args: { page: "bluetooth" } } },
  { id: "display_settings", label: "Display settings", names: ["display settings", "screen settings"], help: "Open Windows display settings.", action: { id: "open_settings", args: { page: "display" } } },
  { id: "lock", label: "Lock Windows", names: ["lock", "lock screen", "lock computer"], help: "Lock your Windows session.", action: { id: "lock", args: {} } },
  { id: "sleep", label: "Sleep", names: ["sleep", "sleep computer"], help: "Put the computer to sleep. Explicit confirmation required.", action: { id: "sleep", args: {} } },
] as const satisfies readonly Definition[];

export type SystemCommandId = typeof systemCommandRegistry[number]["id"];
type Details = { readonly commandId: SystemCommandId; readonly label: string; readonly description: string; readonly query: string; readonly partial?: boolean };
export type SystemSuggestion = Details & (
  | { readonly status: "ready"; readonly action: SystemAction; readonly requiresConfirmation: boolean }
  | { readonly status: "incomplete" | "invalid"; readonly action: null; readonly requiresConfirmation: false }
);
export type SystemParseResult = SystemSuggestion | { readonly status: "none" };

const whole = (value: unknown, low: number, high: number): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= low && value <= high;

/** Validate typed objects as well as parser output. This is not a native execution boundary. */
export function isSystemAction(value: unknown): value is SystemAction {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const object = value as Record<string, unknown>;
  if (Object.keys(object).sort().join(",") !== "args,id" || !object.args || typeof object.args !== "object" || Array.isArray(object.args)) return false;
  const args = object.args as Record<string, unknown>;
  const keys = Object.keys(args).sort().join(",");
  switch (object.id) {
    case "set_volume": return keys === "percent" && whole(args.percent, 0, 100);
    case "adjust_volume": return keys === "delta" && whole(args.delta, -100, 100);
    case "set_mute": return keys === "muted" && typeof args.muted === "boolean";
    case "open_settings": return keys === "page" && typeof args.page === "string" && ["sound", "wifi", "bluetooth", "display"].includes(args.page);
    case "get_audio_state": case "lock": case "sleep": return keys === "";
    default: return false;
  }
}

export function requiresSystemConfirmation(action: SystemAction): boolean { return action.id === "sleep"; }

function ready(details: Details, action: SystemAction): SystemSuggestion {
  return { ...details, status: "ready", action, requiresConfirmation: requiresSystemConfirmation(action) };
}

/** Pure, anchored grammar: no OS calls, evaluation, URI construction, or execution. */
export function parseSystemCommand(input: string): SystemParseResult {
  const query = normalizeSearch(input);
  if (!query) return { status: "none" };
  // Prefer complete aliases before matching a shorter command plus arguments.
  const definition = systemCommandRegistry.find((entry) => entry.names.some((name) => name === query))
    ?? systemCommandRegistry.find((entry) => entry.names.some((name) => query.startsWith(name + " ")));
  if (!definition) return { status: "none" };
  const details: Details = { commandId: definition.id, label: definition.label, description: definition.help, query };
  if (query.length > 256 || /[\u0000-\u0008\u000e-\u001f\u007f-\u009f]/u.test(input)) {
    return { ...details, description: "Use a short command without control characters.", status: "invalid", action: null, requiresConfirmation: false };
  }
  if (definition.id !== "volume") {
    if (definition.names.some((name) => name === query)) return ready(details, definition.action);
    return { ...details, description: `Use ${definition.names[0]} without additional arguments.`, status: "invalid", action: null, requiresConfirmation: false };
  }
  const argument = query.split(" ").slice(1).join(" ");
  if (!argument || argument === "+" || argument === "-") {
    return { ...details, status: "incomplete", action: null, requiresConfirmation: false };
  }
  const numeric = argument === "up" ? "+10" : argument === "down" ? "-10" : argument;
  // Whole decimal numbers only: reject parseInt prefixes, exponents, hex, NaN,
  // Infinity, fractional values, unit suffixes, and additional tokens.
  if (!/^[+-]?\d{1,3}$/.test(numeric)) {
    return { ...details, description: "Volume needs a whole number: 0–100 or a signed change from -100 to +100.", status: "invalid", action: null, requiresConfirmation: false };
  }
  const value = Number(numeric);
  const relative = /^[+-]/.test(numeric);
  if (!whole(value, relative ? -100 : 0, 100)) {
    return { ...details, description: "Volume must be 0–100; changes must be between -100 and +100 percentage points.", status: "invalid", action: null, requiresConfirmation: false };
  }
  return ready({ ...details, label: relative ? `${value < 0 ? "Decrease" : "Increase"} volume by ${Math.abs(value)} points` : `Set volume to ${value}%`,
    description: relative ? "Adjust the current volume by this many percentage points." : "Set the default audio output volume." },
    relative ? { id: "adjust_volume", args: { delta: value === 0 ? 0 : value } } : { id: "set_volume", args: { percent: value } });
}

export function suggestSystemCommands(input: string): readonly SystemSuggestion[] {
  const parsed = parseSystemCommand(input);
  if (parsed.status !== "none") return [parsed];
  const query = normalizeSearch(input);
  // No empty-query takeover and no fuzzy inference of a system action.
  if (!query || query.length > 256) return [];
  return systemCommandRegistry.filter((entry) => entry.names.some((name) => name.startsWith(query))).map((entry) => {
    const details = { commandId: entry.id, label: entry.label, query, description: entry.help, partial: true };
    return entry.id === "volume"
      ? { ...details, status: "incomplete", action: null, requiresConfirmation: false }
      : ready(details, entry.action);
  });
}

export function systemPreviewMessage(suggestion: SystemSuggestion): string {
  if (suggestion.status !== "ready") return suggestion.description;
  return suggestion.description;
}
