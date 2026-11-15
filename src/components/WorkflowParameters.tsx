import { useEffect, useRef, useState } from "react";
import type { CatalogApp } from "../appCatalog";
import type { ActionParameter, WorkflowActionDescriptor } from "../services/workflowActionService";
import type { StudioService, WorkflowStep } from "../services/workflowStudioService";
import { studioError } from "../studioNavigation";
import type { DraftErrors } from "../workflowDraft";

interface Props {
  step: WorkflowStep; descriptor: WorkflowActionDescriptor; errors: DraftErrors;
  apps: readonly CatalogApp[]; appError: string; appsLoading: boolean; refreshApps: () => void;
  service: StudioService; change: (key: string, value: unknown) => void;
}
function ApplicationPicker({ id, value, change, apps, appError, appsLoading, refreshApps }: {
  id: string; value: unknown; change: (value: string) => void;
} & Pick<Props, "apps" | "appError" | "appsLoading" | "refreshApps">) {
  const [query, setQuery] = useState("");
  const matches = apps.filter(a => a.display_name.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const selected = apps.find(a => a.id === value);
  return <div className="studio-app-picker">
    <input aria-label="Search installed applications" placeholder="Search installed applications" value={query} onChange={e => setQuery(e.target.value)} />
    <select id={id} value={typeof value === "string" ? value : ""} onChange={e => change(e.target.value)}>
      <option value="">Select an installed application</option>
      {typeof value === "string" && value && !matches.slice(0, 80).some(a => a.id === value) && <option value={value}>{selected?.display_name ?? "Unavailable application — select another"}</option>}
      {matches.slice(0, 80).map(a => <option key={a.id} value={a.id}>{a.display_name}</option>)}
    </select>
    {!appsLoading && typeof value === "string" && value && !selected && <p className="studio-warning">Saved application unavailable. Select an installed application to repair this step.</p>}
    <small>{appsLoading ? "Loading applications…" : `${matches.length} matches${matches.length > 80 ? "; showing first 80 — narrow your search" : ""}`}</small>
    {appError && <p role="alert" className="studio-error">{appError}</p>}
    <button type="button" onClick={refreshApps} disabled={appsLoading}>Refresh applications</button>
  </div>;
}
function PathPicker({ p, id, value, service, change }: { p: ActionParameter; id: string; value: unknown;
  service: StudioService; change: (value: string) => void }) {
  const [picking, setPicking] = useState(false), [error, setError] = useState("");
  const alive = useRef(true), gate = useRef(false);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  async function pick() {
    if (gate.current || (p.type !== "file" && p.type !== "folder")) return;
    gate.current = true; setPicking(true); setError("");
    try {
      const path = await service.pickPath(p.type);
      if (alive.current && path !== null) change(path);
    } catch (e) { if (alive.current) setError(studioError(e)); }
    finally { gate.current = false; if (alive.current) setPicking(false); }
  }
  return <div className="studio-path-picker">
    <input id={id} value={typeof value === "string" ? value : ""} readOnly placeholder={`Choose a ${p.type}`} />
    <button type="button" disabled={picking} onClick={() => void pick()}>{picking ? "Choosing…" : `Choose ${p.type}`}</button>
    {!p.required && value !== undefined && <button type="button" onClick={() => change("")}>Clear</button>}
    {error && <p role="alert" className="studio-error">{error}</p>}
  </div>;
}
export function WorkflowParameters(props: Props) {
  const { step, descriptor, errors, change } = props;
  return <div className="studio-parameters">{descriptor.parameters.map(p => {
    const id = `parameter-${step.id}-${p.key}`, error = errors[`${step.id}.${p.key}`];
    const value = step.parameters[p.key];
    return <div className="studio-field" key={p.key}>
      <label htmlFor={id}>{p.label}{p.required ? " *" : " (optional)"}</label>
      {p.type === "text" && <input id={id} value={typeof value === "string" ? value : ""}
        onChange={e => change(p.key, e.target.value)} aria-invalid={!!error} aria-describedby={error ? `${id}-error` : undefined} />}
      {p.type === "number" && <><input id={id} type="text" inputMode={p.integer ? "numeric" : "decimal"}
        value={typeof value === "number" || typeof value === "string" ? value : ""} placeholder={`${p.min}–${p.max}`}
        onChange={e => change(p.key, e.target.value)} aria-invalid={!!error} aria-describedby={`${id}-hint${error ? ` ${id}-error` : ""}`} />
        <small id={`${id}-hint`}>{p.integer ? "Whole number" : "Number"} from {p.min} to {p.max}</small></>}
      {p.type === "boolean" && <div>
        {!p.required && <label><input type="checkbox" checked={value !== undefined}
          onChange={e => change(p.key, e.target.checked ? p.default ?? false : undefined)} /> Include value</label>}
        <input id={id} type="checkbox" checked={value === true} disabled={!p.required && value === undefined}
          onChange={e => change(p.key, e.target.checked)} aria-invalid={!!error} />
      </div>}
      {p.type === "choice" && <select id={id} value={typeof value === "string" ? value : ""} onChange={e => change(p.key, e.target.value)} aria-invalid={!!error}>
        <option value="">Choose {p.label}</option>
        {typeof value === "string" && value && !p.options.includes(value) && <option value={value}>Unavailable saved value: {value}</option>}
        {p.options.map(o => <option key={o} value={o}>{o}</option>)}
      </select>}
      {p.type === "application" && <ApplicationPicker {...props} id={id} value={value} change={value => change(p.key, value)} />}
      {(p.type === "file" || p.type === "folder") && <PathPicker p={p} id={id} value={value} service={props.service} change={value => change(p.key, value)} />}
      {error && <p id={`${id}-error`} role="alert" className="studio-error">{error}</p>}
    </div>;
  })}</div>;
}
