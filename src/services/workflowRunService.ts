export type RunStatus = "pending" | "running" | "confirmation-required" | "succeeded" | "failed" | "cancelled";
export interface RunFailure { code: string; message: string; details: unknown }
export interface RunStep {
  step_id: string; action_id: string; status: RunStatus; error: RunFailure | null;
  output: unknown;
}
export interface RunConfirmation {
  token: string; step_id: string; message: string; expires_in_ms: number;
  action: { action_id: string; schema_version: number; parameters: Record<string, unknown>; terminal: boolean; confirmation: string };
}
export interface WorkflowRun {
  run_id: string; workflow_id: string; workflow_revision: number | null; sequence: number;
  status: RunStatus; cancel_requested: boolean; action_in_flight: boolean;
  steps: RunStep[]; confirmation: RunConfirmation | null; error: RunFailure | null;
}
export interface WorkflowProgressEvent { run_id: string; sequence: number }
type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export const runActive = (run: WorkflowRun) => !["succeeded", "failed", "cancelled"].includes(run.status);
export function workflowError(error: unknown): string {
  return error && typeof error === "object" && "message" in error ? String(error.message) : String(error);
}

/** Retained Rust snapshots are authoritative. Presentation lifetime never cancels a run. */
export function createWorkflowRuns(invoke: Invoke) {
  let snapshot = { runs: [] as readonly WorkflowRun[], error: "", busy: [] as readonly string[] };
  const listeners = new Set<() => void>();
  const starting = new Set<string>();
  const mutations = new Set<string>();
  const reads = new Map<string, Promise<void>>();
  const publish = (next: typeof snapshot) => { snapshot = next; listeners.forEach(fn => fn()); };
  const fail = (error: unknown) => publish({ ...snapshot, error: workflowError(error) });
  const merge = (runs: readonly WorkflowRun[]) => {
    const byId = new Map(snapshot.runs.map(run => [run.run_id, run]));
    for (const run of runs) {
      const previous = byId.get(run.run_id);
      if (!previous || run.sequence >= previous.sequence) byId.set(run.run_id, run);
    }
    while (byId.size > 128) {
      const oldestTerminal = [...byId.values()].find(run => !runActive(run));
      if (!oldestTerminal) break;
      byId.delete(oldestTerminal.run_id);
    }
    publish({ ...snapshot, runs: [...byId.values()] });
  };
  const read = (key: string, operation: () => Promise<readonly WorkflowRun[]>) => {
    const pending = reads.get(key);
    if (pending) return pending;
    const request = operation().then(merge, fail).finally(() => { reads.delete(key); });
    reads.set(key, request);
    return request;
  };
  const refresh = () => read("list", () => invoke<WorkflowRun[]>("list_workflow_runs"));
  const status = (runId: string) => read(runId, async () => [await invoke<WorkflowRun>("workflow_run_status", { runId })]);
  async function mutate(runId: string, command: string, args: Record<string, unknown>) {
    if (mutations.has(runId)) return;
    mutations.add(runId); publish({ ...snapshot, busy: [...mutations], error: "" });
    try { merge([await invoke<WorkflowRun>(command, { runId, ...args })]); }
    catch (error) { fail(error); }
    finally { mutations.delete(runId); publish({ ...snapshot, busy: [...mutations] }); }
  }
  return {
    getSnapshot: () => snapshot,
    subscribe: (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; },
    refresh, status,
    event: (event: WorkflowProgressEvent) => {
      const previous = snapshot.runs.find(run => run.run_id === event.run_id);
      if (!previous || event.sequence > previous.sequence) return status(event.run_id);
      return Promise.resolve();
    },
    async start(workflowId: string) {
      if (starting.has(workflowId) || snapshot.runs.some(run => run.workflow_id === workflowId && runActive(run))) {
        throw new Error("This workflow already has an active run.");
      }
      starting.add(workflowId);
      try {
        const run = await invoke<WorkflowRun>("start_workflow", { workflowId });
        merge([run]);
        // Do not wait for completion or let a lost status response turn acceptance into a retry.
        void status(run.run_id);
        return run;
      } catch (error) {
        await refresh(); // Recover an accepted start whose response was lost; never auto-retry.
        throw new Error(workflowError(error) + " Check workflow runs before trying again.");
      } finally { starting.delete(workflowId); }
    },
    cancel: (runId: string) => mutate(runId, "cancel_workflow_run", {}),
    decide: (runId: string, prompt: RunConfirmation, decision: "approve" | "deny") =>
      mutate(runId, "decide_workflow_action", { stepId: prompt.step_id, token: prompt.token, decision }),
    reportError: fail,
  };
}
