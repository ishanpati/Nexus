import { useEffect, useMemo, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { createWorkflowSearchStore } from "../workflowSearch";
import { createWorkflowRuns, runActive, type WorkflowProgressEvent } from "../services/workflowRunService";
import type { WorkflowView } from "../services/workflowStudioService";

export function useLauncherWorkflows() {
  const repository = useMemo(() => createWorkflowSearchStore(() => invoke<WorkflowView[]>("list_workflows")), []);
  const runs = useMemo(() => createWorkflowRuns(invoke), []);
  const saved = useSyncExternalStore(repository.subscribe, repository.getSnapshot);
  const execution = useSyncExternalStore(runs.subscribe, runs.getSnapshot);
  useEffect(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const refresh = () => { if (!disposed) { void repository.refresh(); void runs.refresh(); } };
    const install = async () => {
      const current = getCurrentWindow();
      const track = async (pending: Promise<UnlistenFn>) => {
        const unlisten = await pending;
        if (disposed) unlisten(); else unlisteners.push(unlisten);
      };
      try {
        await track(current.listen("workflows-changed", () => { if (!disposed) void repository.refresh(); }));
        await track(current.listen<WorkflowProgressEvent>("workflow-progress", ({ payload }) => { if (!disposed) void runs.event(payload); }));
        await track(current.listen("launcher-shown", refresh));
        await track(current.onFocusChanged(({ payload }) => { if (payload) refresh(); }));
      } catch (error) { if (!disposed) runs.reportError("Workflow event tracking failed: " + String(error)); }
      refresh(); // Listener first, authoritative list second (also works after reconnect).
    };
    void install();
    const timer = window.setInterval(() => {
      if (runs.getSnapshot().runs.some(runActive)) void runs.refresh();
    }, 1000);
    return () => { disposed = true; unlisteners.forEach(fn => fn()); window.clearInterval(timer); };
  }, [repository, runs]);
  return { saved, repository, execution, runs };
}
