import type { WorkflowView } from "../services/workflowStudioService";
import { runActive, type WorkflowRun, type RunConfirmation } from "../services/workflowRunService";

export function WorkflowProgress({ runs, saved, selectedId, busy, error, onSelect, onClose, onCancel, onDecide, onRefresh }: {
  runs: readonly WorkflowRun[]; saved: readonly WorkflowView[]; selectedId: string | null;
  busy: readonly string[]; error: string; onSelect: (id: string) => void; onClose: () => void;
  onCancel: (id: string) => void; onDecide: (id: string, prompt: RunConfirmation, decision: "approve" | "deny") => void;
  onRefresh: () => void;
}) {
  const run = runs.find(item => item.run_id === selectedId) ?? runs[runs.length - 1];
  const name = (item: WorkflowRun) => saved.find(view => view.workflow.id === item.workflow_id)?.workflow.name ?? item.workflow_id;
  const completed = run?.steps.filter(step => step.status === "succeeded").length ?? 0;
  const pending = !!run && busy.includes(run.run_id);
  return <section className="workflow-progress" aria-label="Workflow runs">
    <div className="workflow-toolbar"><strong>Workflow runs</strong>
      {run && runActive(run) && <button disabled={pending || run.cancel_requested} onClick={() => onCancel(run.run_id)}>
        {run.cancel_requested ? "Cancellation requested" : "Cancel workflow"}</button>}
      <button onClick={onRefresh}>Refresh status</button>
      <button onClick={onClose}>Close progress</button></div>
    {error && <p className="error" role="alert">{error}</p>}
    {!run ? <p>No workflow runs in this session.</p> : <>
      <select aria-label="Select workflow run" value={run.run_id} onChange={event => onSelect(event.currentTarget.value)}>
        {[...runs].reverse().map(item => <option key={item.run_id} value={item.run_id}>{name(item)} · {item.status} · {item.run_id.slice(0, 8)}</option>)}
      </select>
      <p role="status"><strong>{run.status === "succeeded" ? "Completed" : run.status === "failed" ? "Failed" : run.status === "cancelled" ? "Cancelled" : run.status === "confirmation-required" ? "Approval required" : run.status === "pending" ? "Checking workflow…" : "Running"}</strong>
        {` · ${completed}/${run.steps.length} steps completed`}</p>
      {run.error && <p className="error" role="alert">{run.error.message} ({run.error.code})</p>}
      {run.confirmation && <div className="workflow-approval" aria-label="Sensitive action approval">
        <strong>{run.confirmation.message}</strong>
        <p>Step {run.steps.findIndex(step => step.step_id === run.confirmation?.step_id) + 1} · {run.confirmation.action.action_id} · this computer</p>
        <small>Step ID: {run.confirmation.step_id} · schema {run.confirmation.action.schema_version}</small>
        <pre>{JSON.stringify(run.confirmation.action.parameters, null, 2)}</pre>
        <p>Approval expires in about {Math.ceil(run.confirmation.expires_in_ms / 1000)} seconds. Closing progress does not approve or cancel.</p>
        <button disabled={pending} onClick={() => onDecide(run.run_id, run.confirmation!, "deny")}>Deny action</button>{" "}
        <button disabled={pending || run.cancel_requested || run.confirmation.expires_in_ms === 0}
          onKeyDown={event => { if (event.repeat) event.preventDefault(); }}
          onClick={() => onDecide(run.run_id, run.confirmation!, "approve")}>Approve {run.confirmation.action.action_id}</button>
      </div>}
      {runActive(run) && <p>{run.action_in_flight ? "The current Windows request cannot be recalled. Cancellation stops subsequent steps." : "Cancellation stops between steps. Completed actions are not undone."}</p>}
      <ol>{run.steps.map((step, index) => <li key={step.step_id}>
        <span>{index + 1}. {step.action_id} · {step.status === "running" ? "Current step" : step.status}</span>
        {step.status === "succeeded" && step.action_id === "app.launch" && <span> — Windows accepted launch; application readiness unverified.</span>}
        {step.error && <p className="error">Step {index + 1}: {step.error.message} ({step.error.code})</p>}
      </li>)}</ol>
      {run.status === "succeeded" && <p>All steps finished. Application launches report acceptance, not loading completion.</p>}
      {run.status === "failed" && <p>Execution stopped. Earlier actions are not undone; inspect completed steps before starting again.</p>}
      <details><summary>Run details</summary><small>Run {run.run_id} · Workflow {run.workflow_id}{run.workflow_revision !== null ? ` · revision ${run.workflow_revision}` : ""}</small></details>
    </>}
  </section>;
}
