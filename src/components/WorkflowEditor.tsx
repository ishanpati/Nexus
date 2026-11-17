import { launcherKeys } from "../launcherCapabilities";
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CatalogApp } from "../appCatalog";
import { createWorkflowActionCatalog, type WorkflowActionDescriptor } from "../services/workflowActionService";
import type { StudioService, WorkflowView } from "../services/workflowStudioService";
import { studioError } from "../studioNavigation";
import { createDraft, descriptorProblem, draftFingerprint, initialParameters, newStep, removeStep, reorderSteps, stepSummary, validateDraft, type WorkflowDraft } from "../workflowDraft";
import { WorkflowParameters } from "./WorkflowParameters";

const icons: Record<string, string> = { application: "▣", file: "▤", folder: "▱", volume: "◖", settings: "⚙", lock: "▢", moon: "☾" };
export interface WorkflowEditorHandle { save: () => Promise<boolean> }
export function WorkflowEditor({ initial, disabled, service, onDirty, onSaving, onSaved, onName, ref, onReload }: {
  initial?: WorkflowView; disabled: boolean; service: StudioService; onDirty: (dirty: boolean) => void;
  onSaving: (saving: boolean) => void; onSaved: (view: WorkflowView) => void; onName: (name: string) => void;
  ref?: Ref<WorkflowEditorHandle>; onReload: () => void;
}) {
  const [draft, setDraft] = useState(() => createDraft(initial));
  const draftRef = useRef(draft);
  const baseline = useRef(JSON.stringify(draft)), revision = useRef(initial?.workflow.revision ?? null);
  const savedView = useRef(initial);
  const [catalog, setCatalog] = useState<readonly WorkflowActionDescriptor[]>([]);
  const [catalogError, setCatalogError] = useState(""), [catalogLoading, setCatalogLoading] = useState(true);
  const [catalogRetry, setCatalogRetry] = useState(0);
  const [apps, setApps] = useState<readonly CatalogApp[]>([]), [appError, setAppError] = useState("");
  const [appsLoading, setAppsLoading] = useState(true), [appRefresh, setAppRefresh] = useState(0);
  const [picker, setPicker] = useState(false), [query, setQuery] = useState("");
  const [active, setActive] = useState<string | null>(null), [attempted, setAttempted] = useState(false);
  const [saving, setSaving] = useState(false), saveGate = useRef(false);
  const [feedback, setFeedback] = useState(""), [saveError, setSaveError] = useState("");
  const nameInput = useRef<HTMLInputElement>(null), pickerInput = useRef<HTMLInputElement>(null);
  const stepsHeading = useRef<HTMLHeadingElement>(null);
  const activeForm = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (initial && initial.workflow.revision !== revision.current) {
      const next = createDraft(initial);
      draftRef.current = next; baseline.current = JSON.stringify(next); revision.current = initial.workflow.revision; savedView.current = initial;
      setDraft(next); setActive(null); setAttempted(false); setFeedback("");
    }
  }, [initial]);
  useEffect(() => { activeForm.current?.querySelector<HTMLElement>("input, select, button")?.focus(); }, [active]);
  useEffect(() => {
    let disposed = false;
    setCatalogLoading(true); setCatalogError("");
    void createWorkflowActionCatalog(invoke)().then(rows => { if (!disposed) setCatalog(rows); })
      .catch(e => { if (!disposed) setCatalogError(studioError(e)); }).finally(() => { if (!disposed) setCatalogLoading(false); });
    return () => { disposed = true; };
  }, [catalogRetry]);
  useEffect(() => {
    let disposed = false;
    setAppsLoading(true); setAppError("");
    void service.apps(appRefresh > 0).then(result => { if (!disposed) { setApps(result.apps); setAppError(result.warnings.join(" ")); } })
      .catch(e => { if (!disposed) setAppError(studioError(e)); }).finally(() => { if (!disposed) setAppsLoading(false); });
    return () => { disposed = true; };
  }, [service, appRefresh]);
  useEffect(() => { if (picker) pickerInput.current?.focus(); }, [picker]);
  useEffect(() => { nameInput.current?.focus(); }, []);
  function change(update: (draft: WorkflowDraft) => WorkflowDraft) {
    // All draft mutations are synchronous; no side effects inside React updaters.
    if (saveGate.current) return;
    const next = update(draftRef.current);
    draftRef.current = next;
    setDraft(next); onDirty(draftFingerprint(next, catalog) !== baseline.current); onName(next.name);
    setFeedback(""); setSaveError("");
  }
  useImperativeHandle(ref, () => ({ save: () => save(true) }));
  const validation = validateDraft(draft, catalog, savedView.current);
  const errors = attempted ? validation.errors : {};
  const hasStepErrors = (id: string) => Object.keys(validation.errors).some(key => key === id || key.startsWith(`${id}.`));
  async function save(forNavigation = false): Promise<boolean> {
    if (saveGate.current || (disabled && !forNavigation) || catalogLoading || catalogError) return false;
    if (revision.current !== null && draftFingerprint(draftRef.current, catalog) === baseline.current) return true;
    setAttempted(true); setFeedback(""); setSaveError("");
    const checked = validateDraft(draftRef.current, catalog, savedView.current);
    if (Object.keys(checked.errors).length) {
      const bad = draftRef.current.steps.find(s => Object.keys(checked.errors).some(key => key === s.id || key.startsWith(`${s.id}.`)));
      if (bad) setActive(bad.id);
      if (checked.errors.name) nameInput.current?.focus();
      return false;
    }
    saveGate.current = true; setSaving(true); onSaving(true);
    try {
      const view = await service.save(checked.payload, revision.current);
      const next = createDraft(view);
      revision.current = view.workflow.revision; baseline.current = JSON.stringify(next); savedView.current = view;
      draftRef.current = next; setDraft(next); onDirty(false); onSaved(view); onName(next.name); setAttempted(false);
      setFeedback(`Workflow saved · Revision ${view.workflow.revision}`);
      return true;
    } catch (e) { setSaveError(studioError(e)); return false; }
    finally { saveGate.current = false; setSaving(false); onSaving(false); }
  }
  const matching = catalog.filter(d => d.workflow_compatible && `${d.display_name} ${d.description} ${d.category}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const categories = [...new Set(matching.map(d => d.category))];
  return <form onSubmit={e => { e.preventDefault(); void save(); }} noValidate>
    <fieldset disabled={disabled || saving} className="studio-editor">
      <div className="studio-field"><label htmlFor="workflow-name">Workflow name *</label>
        <input ref={nameInput} id="workflow-name" value={draft.name} onChange={e => change(d => ({ ...d, name: e.target.value }))} aria-invalid={!!errors.name} aria-describedby={errors.name ? "workflow-name-error" : undefined} />
        {errors.name && <p id="workflow-name-error" role="alert" className="studio-error">{errors.name}</p>}</div>
      <div className="studio-field"><label htmlFor="workflow-description">Description (optional)</label>
        <textarea id="workflow-description" rows={2} value={draft.description} onChange={e => change(d => ({ ...d, description: e.target.value }))} aria-invalid={!!errors.description} />
        {errors.description && <p role="alert" className="studio-error">{errors.description}</p>}</div>
      <div className="studio-field"><label htmlFor="workflow-aliases">Aliases (optional, one per line)</label>
        <textarea id="workflow-aliases" rows={2} value={draft.aliases} onChange={e => change(d => ({ ...d, aliases: e.target.value }))} aria-invalid={!!errors.aliases} />
        {errors.aliases && <p role="alert" className="studio-error">{errors.aliases}</p>}</div>
      <div className="studio-steps-header"><h3 ref={stepsHeading} tabIndex={-1}>Actions <small>{draft.steps.length}/64</small></h3>
        <button type="button" disabled={catalogLoading || !!catalogError || draft.steps.length >= 64} onClick={() => setPicker(!picker)} aria-expanded={picker}>Add Action</button></div>
      {catalogLoading && <p role="status">Loading action registry…</p>}
      {catalogError && <p role="alert" className="studio-error">{catalogError} <button type="button" onClick={() => setCatalogRetry(n => n + 1)}>Retry registry</button></p>}
      {picker && <div className="studio-action-picker" aria-label="Choose an action">
        <label htmlFor="action-search">Search actions</label><input id="action-search" ref={pickerInput} value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => { if (e.key === launcherKeys.dismiss) { e.preventDefault(); setPicker(false); } }} />
        {!matching.length && <p>No matching workflow actions.</p>}
        {categories.map(category => <div key={category}><h4>{category}</h4><ul>{matching.filter(d => d.category === category).map(d => {
          const problem = descriptorProblem(d) || (d.terminal && draft.steps.some(s => catalog.find(a => a.id === s.action_id)?.terminal) ? "A final action is already present. Remove it before choosing another." : "");
          return <li key={d.id}><button type="button" disabled={!!problem} onClick={() => {
            const step = newStep(d, draft.steps.length);
            // Inserting before a terminal step keeps it final.
            change(w => { const steps = [...w.steps]; const at = steps.findIndex(s => catalog.find(a => a.id === s.action_id)?.terminal);
              steps.splice(at < 0 ? steps.length : at, 0, step); return { ...w, steps: steps.map((s, order) => ({ ...s, order })) }; });
            setActive(step.id); setPicker(false); setQuery("");
          }}><span aria-hidden="true">{icons[d.icon] ?? "◇"}</span><span><strong>{d.display_name}</strong><small>{d.description}</small></span></button>
            {problem && <p className="studio-warning">Unavailable: {problem}</p>}</li>;
        })}</ul></div>)}
        <button type="button" onClick={() => setPicker(false)}>Close action list</button>
      </div>}
      {errors.steps && <p role="alert" className="studio-error">{errors.steps}</p>}
      {!draft.steps.length && <p className="studio-empty">Add your first action to build a workflow.</p>}
      <ol className="studio-steps">{draft.steps.map((step, index) => {
        const d = catalog.find(d => d.id === step.action_id);
        const problem = !d ? `Action ${step.action_id} is unavailable.` : descriptorProblem(d) || (step.action_schema_version !== d.schema_version ? "Unsupported saved action schema version." : "");
        const terminal = !!d?.terminal;
        const upBlocked = index === 0 || terminal || !!catalog.find(d => d.id === draft.steps[index - 1]?.action_id)?.terminal;
        const downBlocked = index === draft.steps.length - 1 || terminal || !!catalog.find(d => d.id === draft.steps[index + 1]?.action_id)?.terminal;
        const label = d?.display_name ?? step.action_id;
        return <li key={step.id} className="studio-step">
          <div className="studio-step-title"><span aria-hidden="true">{d ? icons[d.icon] ?? "◇" : "◇"}</span><strong>{index + 1}. {step.label || label}</strong></div>
          <p>{stepSummary(step, d, apps)}</p>
          {!!problem && <><p className="studio-warning" role="alert">Unavailable: {problem} Saved parameters are preserved. This workflow cannot run until the action is repaired or removed.</p>
            <details><summary>Saved action and parameters</summary><p>{step.action_id} · Schema {step.action_schema_version}</p><pre>{JSON.stringify(step.parameters, null, 2)}</pre></details></>}
          {d?.confirmation !== "none" && d && <p className="studio-warning">{d.confirmation === "always" ? "Requires explicit confirmation when run." : "Targets are checked again when run. Executable, script and unknown file types cannot run in workflows."} {terminal && "Must be the final step."}</p>}
          {errors[step.id] && <p role="alert" className="studio-error">{errors[step.id]}</p>}
          {attempted && hasStepErrors(step.id) && active !== step.id && <p className="studio-error">This action needs attention. Open Edit to review its fields.</p>}
          <div className="studio-step-buttons"><button type="button" disabled={!!problem} aria-label={`Edit ${label}, step ${index + 1}`} aria-expanded={active === step.id} onClick={() => setActive(active === step.id ? null : step.id)}>Edit</button>
            {d && !descriptorProblem(d) && (step.action_schema_version !== d.schema_version || Object.keys(step.parameters).some(key => !d.parameters.some(p => p.key === key))) &&
              <button type="button" onClick={() => { change(w => ({ ...w, steps: w.steps.map(s => s.id === step.id ? { ...s, action_schema_version: d.schema_version, parameters: initialParameters(d) } : s) })); setActive(step.id); }}>Reset to current action fields</button>}
            <button type="button" aria-label={`Remove ${label}, step ${index + 1}`} onClick={() => { change(w => ({ ...w, steps: removeStep(w.steps, step.id) })); if (active === step.id) setActive(null); stepsHeading.current?.focus(); }}>Remove</button>
            <button type="button" disabled={upBlocked} aria-label={`Move Up ${label}, step ${index + 1}`} onClick={() => change(w => ({ ...w, steps: reorderSteps(w.steps, step.id, -1) }))}>Move Up</button>
            <button type="button" disabled={downBlocked} aria-label={`Move Down ${label}, step ${index + 1}`} onClick={() => change(w => ({ ...w, steps: reorderSteps(w.steps, step.id, 1) }))}>Move Down</button></div>
          {active === step.id && d && !problem && <div className="studio-step-form" ref={activeForm}>
            <h4>Configure {label}</h4>
            <WorkflowParameters step={step} descriptor={d} service={service} errors={errors} apps={apps} appError={appError} appsLoading={appsLoading} refreshApps={() => setAppRefresh(n => n + 1)}
              change={(key, value) => change(w => ({ ...w, steps: w.steps.map(s => s.id === step.id ? { ...s, parameters: { ...s.parameters, [key]: value } } : s) }))} />
            {!d.parameters.length && <p>No configuration required.</p>}
            <button type="button" onClick={() => { setAttempted(true); if (!hasStepErrors(step.id)) setActive(null); }}>Done</button>
          </div>}
        </li>;
      })}</ol>
      <div className="studio-save"><button type="submit" disabled={saving || catalogLoading || !!catalogError || (revision.current !== null && draftFingerprint(draft, catalog) === baseline.current)}>{saving ? "Saving…" : "Save Workflow"}</button>
        <small>Saved locally. Actions run only when explicitly started.</small></div>
    </fieldset>
    {saveError && <p role="alert" className="studio-error">{saveError}</p>}
    {saveError && revision.current !== null && <button type="button" disabled={disabled || saving} onClick={onReload}>Reload saved workflow</button>}
    {feedback && <p role="status" className="studio-success">{feedback}</p>}
    {initial?.issues.map((issue, i) => <p key={i} className="studio-warning">Stored workflow: {issue}</p>)}
  </form>;
}
