import type { StudioRoute } from "../studioNavigation.ts";
import type { AppCatalog } from "../appCatalog.ts";
export interface WorkflowStep {
  id: string; action_id: string; action_schema_version: number;
  parameters: Record<string, unknown>; order: number; label: string | null;
}
export interface WorkflowPayload {
  id: string; name: string; description: string | null; aliases: string[];
  steps: WorkflowStep[]; format_version: number;
}
export interface WorkflowView {
  workflow: WorkflowPayload & {
    revision: number; created_at: number; updated_at: number;
  };
  executable: boolean; issues: string[];
}
type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function createStudioService(invoke: Invoke) {
  // Synchronous gate also covers two activations before React can disable Save.
  let mutation: "save" | "delete" | null = null;
  return {
    list: () => invoke<WorkflowView[]>("list_workflows"),
    get: (id: string) => invoke<WorkflowView>("get_workflow", { id }),
    open: (route: StudioRoute) => invoke<void>("open_workflow_studio", { route }),
    apps: (refresh = false) => invoke<AppCatalog>(refresh ? "refresh_installed_apps" : "list_installed_apps"),
    pickPath: (kind: "file" | "folder") => invoke<string | null>("pick_workflow_path", { kind }),
    save: async (payload: WorkflowPayload, expectedRevision: number | null) => {
      if (mutation) throw new Error(`A ${mutation} is already in progress.`);
      mutation = "save";
      try { return await invoke<WorkflowView>("save_workflow", { payload, expectedRevision }); }
      finally { mutation = null; }
    },
    delete: async (id: string, expectedRevision: number, confirmed: boolean) => {
      if (!confirmed) throw new Error("Confirm deletion of this workflow ID and revision.");
      if (mutation) throw new Error(`A ${mutation} is already in progress.`);
      mutation = "delete";
      try { await invoke<void>("delete_workflow", { id, expectedRevision, confirmed }); }
      finally { mutation = null; }
    },
  };
}
export type StudioService = ReturnType<typeof createStudioService>;
