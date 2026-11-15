import type { WorkflowView } from "./services/workflowStudioService.ts";

export function filterWorkflows(items: readonly WorkflowView[], query: string): WorkflowView[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return items.filter(({ workflow: w }) => {
    const text = [w.name, w.description ?? "", ...w.aliases].join(" ").toLocaleLowerCase();
    return words.every(word => text.includes(word));
  }).sort((a, b) => b.workflow.updated_at - a.workflow.updated_at || a.workflow.name.localeCompare(b.workflow.name));
}
