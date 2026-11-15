import type { ActionParameter, WorkflowActionDescriptor } from "./services/workflowActionService.ts";
import type { WorkflowPayload, WorkflowStep, WorkflowView } from "./services/workflowStudioService.ts";

export interface WorkflowDraft extends Omit<WorkflowPayload, "description" | "aliases"> {
  description: string; aliases: string;
}
export function createDraft(view?: WorkflowView): WorkflowDraft {
  const w = view?.workflow;
  return { id: w?.id ?? crypto.randomUUID(), name: w?.name ?? "", description: w?.description ?? "",
    aliases: w?.aliases.join("\n") ?? "", format_version: w?.format_version ?? 1,
    steps: w ? [...w.steps].sort((a, b) => a.order - b.order).map(s => ({ ...s, parameters: { ...s.parameters } })) : [] };
}

/** Fail closed for future metadata that this editor cannot represent. */
export function descriptorProblem(d: WorkflowActionDescriptor): string {
  if (!d.workflow_compatible) return "This action is unavailable in workflows.";
  if (!Number.isSafeInteger(d.schema_version) || d.schema_version < 1 || !Array.isArray(d.parameters)
      || !["none", "sensitive_target", "always"].includes(d.confirmation)
      || typeof d.terminal !== "boolean" || (d.terminal && d.confirmation !== "always")) return "Unsupported action descriptor.";
  const keys = new Set<string>();
  for (const p of d.parameters) {
    if (!p || !p.key || keys.has(p.key) || typeof p.label !== "string" || typeof p.required !== "boolean") return "Unsupported parameter descriptor.";
    keys.add(p.key);
    switch (p.type) {
      case "text": if (!Number.isSafeInteger(p.max_length) || p.max_length < 1) return "Unsupported text constraints."; break;
      case "number": if (!Number.isFinite(p.min) || !Number.isFinite(p.max) || p.min > p.max || typeof p.integer !== "boolean") return "Unsupported number constraints."; break;
      case "choice": if (!Array.isArray(p.options) || !p.options.length || p.options.some((o: unknown) => typeof o !== "string" || !o)
        || new Set(p.options).size !== p.options.length) return "Unsupported choice options."; break;
      case "boolean": case "application": case "file": case "folder": break;
      default: return `Unsupported parameter type: ${String((p as { type: unknown }).type)}.`;
    }
    if (p.default !== undefined && parameterValue(p, p.default).error) return "Unsupported parameter default.";
  }
  return "";
}
export function initialParameters(d: WorkflowActionDescriptor): Record<string, unknown> {
  return Object.fromEntries(d.parameters.flatMap(p => p.default !== undefined ? [[p.key, p.default]]
    : p.type === "boolean" && p.required ? [[p.key, false]] : []));
}
export function newStep(d: WorkflowActionDescriptor, order: number): WorkflowStep {
  return { id: crypto.randomUUID(), action_id: d.id, action_schema_version: d.schema_version,
    parameters: initialParameters(d), order, label: null };
}
export function reorderSteps(steps: WorkflowStep[], id: string, offset: -1 | 1): WorkflowStep[] {
  const index = steps.findIndex(s => s.id === id), to = index + offset;
  if (index < 0 || to < 0 || to >= steps.length) return steps;
  const next = [...steps];
  [next[index], next[to]] = [next[to], next[index]];
  return next.map((s, order) => ({ ...s, order }));
}
export function removeStep(steps: WorkflowStep[], id: string): WorkflowStep[] {
  return steps.filter(s => s.id !== id).map((s, order) => ({ ...s, order }));
}
const controls = /[\u0000-\u001f\u007f-\u009f]/;
const length = (s: string) => [...s].length;
export function parameterValue(p: ActionParameter, raw: unknown): { value?: unknown; error?: string } {
  if (raw === undefined || raw === "") return p.required ? { error: `${p.label} is required.` } : {};
  switch (p.type) {
    case "number": {
      if (typeof raw !== "number" && (typeof raw !== "string" || !/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(raw))) return { error: "Enter a valid number." };
      const n = Number(raw);
      if (!Number.isFinite(n) || n < p.min || n > p.max || (p.integer && !Number.isSafeInteger(n))) return { error: `Use ${p.integer ? "a whole number" : "a number"} from ${p.min} to ${p.max}.` };
      return { value: n };
    }
    case "boolean": return typeof raw === "boolean" ? { value: raw } : { error: "Choose a boolean value." };
    case "choice": return typeof raw === "string" && p.options.includes(raw) ? { value: raw } : { error: "Choose an allowed value." };
    case "text": case "application": case "file": case "folder": {
      const limit = p.type === "text" ? p.max_length : p.type === "application" ? 131072 : 32767;
      if (typeof raw !== "string" || !raw.trim() || controls.test(raw) || length(raw) > limit) return { error: `Enter ${p.label} (up to ${limit} characters, without control characters).` };
      return { value: raw };
    }
    default: return { error: "Unsupported parameter type." };
  }
}
export type DraftErrors = Record<string, string>;
export function draftFingerprint(draft: WorkflowDraft, catalog: readonly WorkflowActionDescriptor[]): string {
  return JSON.stringify({ ...draft, steps: draft.steps.map(step => {
    const d = catalog.find(d => d.id === step.action_id);
    if (!d || descriptorProblem(d) || d.schema_version !== step.action_schema_version) return step;
    const parameters = { ...step.parameters };
    for (const p of d.parameters) if (p.type === "number" && parameters[p.key] !== undefined) {
      const result = parameterValue(p, parameters[p.key]);
      if (!result.error && result.value !== undefined) parameters[p.key] = result.value;
    }
    return { ...step, parameters };
  }) });
}
export function validateDraft(draft: WorkflowDraft, catalog: readonly WorkflowActionDescriptor[], initial?: WorkflowView) {
  const errors: DraftErrors = {};
  const name = draft.name.trim();
  if (!name || length(name) > 120 || controls.test(name)) errors.name = "Enter a workflow name of 1–120 characters without control characters.";
  if (new TextEncoder().encode(draft.description).length > 4096) errors.description = "Description must be at most 4096 bytes.";
  const aliases = draft.aliases.split(/\r?\n/).map(s => s.trim()).filter(Boolean);
  const names = [name, ...aliases].map(s => s.toLowerCase());
  if (aliases.length > 20 || aliases.some(s => length(s) > 120 || controls.test(s)) || new Set(names).size !== names.length) errors.aliases = "Use up to 20 unique aliases, one per line (1–120 characters each), distinct from the name.";
  if (!draft.steps.length || draft.steps.length > 64) errors.steps = "Add between 1 and 64 complete actions.";
  if (draft.format_version !== 1) errors.steps = "Unsupported workflow format. This workflow cannot be saved by this editor.";
  const steps = draft.steps.map((step, order) => {
    const d = catalog.find(d => d.id === step.action_id);
    const problem = !d ? `Action ${step.action_id} is unavailable.` : descriptorProblem(d)
      || (step.action_schema_version !== d.schema_version ? "This saved action uses an unsupported schema version." : "");
    if (d?.terminal === true && order !== draft.steps.length - 1) errors[step.id] = "This action must be the final step.";
    if (problem) {
      // Match Rust's retention policy: an unavailable reference may survive only
      // with its original identity, version and parameters. Never drop its data.
      const retained = initial?.workflow.id === draft.id && initial.workflow.steps.some(old => old.id === step.id
        && old.action_id === step.action_id && old.action_schema_version === step.action_schema_version
        && JSON.stringify(old.parameters) === JSON.stringify(step.parameters));
      if (!retained) errors[step.id] = problem;
      return { ...step, order };
    }
    if (d!.terminal && order !== draft.steps.length - 1) errors[step.id] = "This action must be the final step.";
    const parameters: Record<string, unknown> = {};
    for (const key of Object.keys(step.parameters)) if (!d!.parameters.some(p => p.key === key)) errors[step.id] = `Unknown parameter: ${key}. Remove or repair this action.`;
    for (const p of d!.parameters) {
      const result = parameterValue(p, step.parameters[p.key] === undefined || step.parameters[p.key] === "" ? p.default : step.parameters[p.key]);
      if (result.error) errors[`${step.id}.${p.key}`] = result.error;
      else if (result.value !== undefined) parameters[p.key] = result.value;
    }
    return { ...step, order, parameters };
  });
  const payload: WorkflowPayload = { id: draft.id, name, description: draft.description || null, aliases, steps, format_version: draft.format_version };
  return { errors, payload };
}
export function stepSummary(step: WorkflowStep, d: WorkflowActionDescriptor | undefined, apps: readonly { id: string; display_name: string }[]): string {
  if (!d || descriptorProblem(d)) return "Unavailable action — parameters preserved.";
  return d.parameters.map(p => {
    const value = step.parameters[p.key];
    const display = value === undefined || value === "" ? "Not set" : p.type === "application"
      ? apps.find(a => a.id === value)?.display_name ?? "Unavailable application" : typeof value === "boolean" ? value ? "Yes" : "No" : String(value);
    return `${p.label}: ${display}`;
  }).join(" · ") || "No configuration required";
}
