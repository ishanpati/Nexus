import { useEffect, useRef, useState } from "react";
import type { StudioService, WorkflowView } from "../services/workflowStudioService";
import { studioError } from "../studioNavigation";
import { filterWorkflows } from "../workflowList";

export function SavedWorkflows({ items, loading, error, disabled, service, onOpen, onCreate, onRefresh, onDeleted, onDeleting }: {
  items: WorkflowView[]; loading: boolean; error: string; disabled: boolean; service: StudioService;
  onOpen: (id: string) => void; onCreate: () => void; onRefresh: () => void;
  onDeleted: (id: string) => void; onDeleting: (value: boolean) => void;
}) {
  const [query, setQuery] = useState("");
  const [target, setTarget] = useState<WorkflowView | null>(null);
  const [deleting, setDeleting] = useState(false), [deleteError, setDeleteError] = useState("");
  const gate = useRef(false), dialog = useRef<HTMLDialogElement>(null);
  const rows = filterWorkflows(items, query);
  useEffect(() => { if (target) dialog.current?.showModal(); else dialog.current?.close(); }, [target]);
  function cancelDelete() { if (!gate.current) { setTarget(null); setDeleteError(""); } }
  async function remove() {
    if (!target || gate.current || disabled) return;
    gate.current = true; setDeleting(true); onDeleting(true); setDeleteError("");
    try {
      await service.delete(target.workflow.id, target.workflow.revision, true);
      setTarget(null); onDeleted(target.workflow.id);
    } catch (e) { setDeleteError(studioError(e)); }
    finally { gate.current = false; setDeleting(false); onDeleting(false); }
  }
  return <>
    <div className="studio-steps-header"><div><h2>Saved workflows</h2><p>Workflows saved on this device.</p></div>
      <button disabled={disabled} onClick={onCreate}>Create New Workflow</button></div>
    <div className="studio-list-tools"><label htmlFor="workflow-search">Search saved workflows</label>
      <input id="workflow-search" type="search" value={query} onChange={e => setQuery(e.target.value)} placeholder="Name, description or alias" />
      <button disabled={disabled || loading} onClick={onRefresh}>Refresh</button></div>
    {!loading && !error && items.length === 0 && <p className="studio-empty">No saved workflows yet. Create a new workflow to get started.</p>}
    {!loading && !error && items.length > 0 && !rows.length && <p className="studio-empty">No workflows match your search.</p>}
    <ul className="studio-list">{rows.map(({ workflow: w, executable, issues }) => <li key={w.id}>
      <div className="studio-workflow-info"><button className="studio-workflow-name" disabled={disabled} onClick={() => onOpen(w.id)}>{w.name}</button>
        <p>{w.description || "No description"}</p>
        <p>{w.steps.length} {w.steps.length === 1 ? "action" : "actions"}
          {w.updated_at > 0 && <> · Updated <time dateTime={new Date(w.updated_at).toISOString()}>{new Date(w.updated_at).toLocaleString()}</time></>}</p>
        {!executable && <details><summary className="studio-warning">Needs attention</summary>{issues.map((issue, i) => <p key={i} className="studio-warning">{issue}</p>)}</details>}</div>
      <div className="studio-step-buttons"><button disabled={disabled} aria-label={`Edit ${w.name}`} onClick={() => onOpen(w.id)}>Edit</button>
        <button disabled={disabled} aria-label={`Delete ${w.name}`} onClick={() => { setTarget(items.find(v => v.workflow.id === w.id)!); setDeleteError(""); }}>Delete</button></div>
    </li>)}</ul>
    <dialog ref={dialog} onCancel={e => { e.preventDefault(); cancelDelete(); }} aria-labelledby="delete-title">
      <h2 id="delete-title">Delete saved workflow?</h2>
      <p>Delete “{target?.workflow.name}” and its {target?.workflow.steps.length} saved actions? This cannot be undone.</p>
      <p className="studio-id">Workflow ID: {target?.workflow.id}<br />Revision: {target?.workflow.revision}</p>
      <div><button autoFocus disabled={deleting} onClick={cancelDelete}>Cancel</button>
        <button disabled={disabled || deleting} onClick={() => void remove()}>{deleting ? "Deleting…" : "Delete workflow"}</button></div>
      {deleteError && <p role="alert" className="studio-error">{deleteError} Cancel and refresh the list before retrying.</p>}
    </dialog>
  </>;
}
