import { indexWorkflows, rankApplications } from "./appSearch.ts";
import { searchMode } from "./fileSearch.ts";
import type { WorkflowView } from "./services/workflowStudioService.ts";
import { workflowError } from "./services/workflowRunService.ts";

export function searchWorkflows(input: string, workflows: readonly WorkflowView[]) {
  const mode = searchMode(input);
  return mode.kind === "workflows" ? rankApplications(indexWorkflows(workflows), mode.query) : [];
}

/** Refreshes invalidate activation immediately; superseded reads cannot restore renamed/deleted rows. */
export function createWorkflowSearchStore(list: () => Promise<WorkflowView[]>) {
  let snapshot = { items: [] as readonly WorkflowView[], loading: true, error: "" };
  const listeners = new Set<() => void>();
  let generation = 0;
  const publish = (next: typeof snapshot) => { snapshot = next; listeners.forEach(fn => fn()); };
  return {
    getSnapshot: () => snapshot,
    subscribe: (fn: () => void) => { listeners.add(fn); return () => { listeners.delete(fn); }; },
    async refresh() {
      const current = ++generation;
      publish({ ...snapshot, loading: true, error: "" });
      try {
        const items = await list();
        if (current === generation) publish({ items, loading: false, error: "" });
      } catch (error) {
        if (current === generation) publish({ items: [], loading: false, error: workflowError(error) });
      }
    },
  };
}
