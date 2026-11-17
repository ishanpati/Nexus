import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { DiscoverableCapability } from "../capabilityMetadata";
import { groupCapabilities } from "../capabilityMetadata";
import { copyHelpExample, describeParameter, filterHelp, helpCategories } from "../helpCenter";
import { createCommandDiscoverySnapshotService } from "../services/commandDiscoveryService";
import "./HelpCenter.css";

const loadMetadata = createCommandDiscoverySnapshotService(invoke);
const kindLabels = { command: "Command", search_mode: "Search mode", interaction: "Control", shortcut: "Shortcut", action: "Workflow action" };
const confirmationText = {
  none: "", always: "Requires explicit confirmation.",
  sensitive_target: "Sensitive targets require confirmation. Workflows reject executable, script and unknown file types.",
  per_step: "Sensitive workflow steps require separate approval for each run.",
};

export function HelpNavigation({ categories, count, selected, onSelect }: {
  categories: readonly (readonly [string, number])[]; count: number; selected: string | null; onSelect: (category: string | null) => void;
}) {
  return <nav className="help-nav" aria-label="Help categories"><span className="help-field-label">EXPLORE</span>
    <button type="button" aria-pressed={selected === null} onClick={() => onSelect(null)}>All capabilities<span>{count}</span></button>
    {categories.map(([name, categoryCount]) => <button type="button" key={name} aria-pressed={selected === name} onClick={() => onSelect(name)}>{name}<span>{categoryCount}</span></button>)}
    <p>Choose a category or search the full reference.</p>
  </nav>;
}

export function HelpCard({ item, onCopy }: { item: DiscoverableCapability; onCopy: (text: string) => void }) {
  const actions = item.actions ?? [];
  const hasDetails = !!item.parameters?.length || actions.some(action => action.parameters?.length || action.confirmation !== "none" || action.terminal);
  return <article className={`help-card help-card-${item.kind}`} aria-label={item.displayName}>
    <div className="help-card-heading"><h3>{item.displayName}</h3><span className="help-kind">{kindLabels[item.kind] ?? "Capability"}</span></div>
    <p className="help-description">{item.description}</p>
    <code className="help-syntax">{item.syntax}</code>
    {!!item.aliases?.length && <p className="help-aliases"><span>Also accepts</span> {item.aliases.map(alias => <code key={alias}>{alias}</code>)}</p>}
    {!!item.examples?.length && <div className="help-examples"><span className="help-field-label">Examples</span>{item.examples.map((example, index) =>
      <div className="help-example" key={`${index}:${example}`}><code>{example}</code><button type="button" aria-label={`Copy example: ${example}`} onClick={() => onCopy(example)}>Copy</button></div>)}</div>}
    {!!item.prerequisites?.length && <div className="help-prerequisites"><span className="help-field-label">Before you start</span><ul>{item.prerequisites.map(text => <li key={text}>{text}</li>)}</ul></div>}
    {item.launcherConfirmation && confirmationText[item.launcherConfirmation] && <p className="help-policy">{confirmationText[item.launcherConfirmation]}</p>}
    {hasDetails && <details className="help-details"><summary>Parameters &amp; workflow details</summary>
      {!!item.parameters?.length && <dl>{item.parameters.map(parameter => <div key={parameter.name}><dt>{parameter.name}</dt><dd>{parameter.description}</dd></div>)}</dl>}
      {actions.map(action => <div className="help-action-details" key={action.id}><strong>{action.display_name}</strong>
        {!!action.parameters?.length && <dl>{action.parameters.map(parameter => <div key={parameter.key}><dt>{parameter.label}</dt><dd>{describeParameter(parameter)}</dd></div>)}</dl>}
        {action.confirmation !== "none" && <p>{action.confirmation === "always" ? "Workflow execution requires fresh confirmation." : "Workflow targets are checked again before execution; executable, script and unknown file types cannot run."}</p>}
        {action.terminal && <p>Must be the final step in a workflow.</p>}
      </div>)}
    </details>}
    <div className="help-card-footer">{item.workflowCompatible && <span>Available in workflows</span>}
      {item.availability?.status === "unavailable" && <span className="help-unavailable">Currently unavailable</span>}
    </div>
    {item.availability?.reason && (item.availability.status !== "unknown" || !!item.prerequisites?.length) && <p className={`help-availability ${item.availability.status === "unavailable" ? "help-unavailable" : ""}`}>
      {item.availability.reason}</p>}
  </article>;
}

export function HelpCenter() {
  const [items, setItems] = useState<readonly DiscoverableCapability[]>([]);
  const [version, setVersion] = useState("");
  const [loading, setLoading] = useState(true), [error, setError] = useState("");
  const [reload, setReload] = useState(0), [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{ text: string; generation: number } | null>(null);
  const input = useRef<HTMLInputElement>(null), panel = useRef<HTMLElement>(null);
  const copyGeneration = useRef(0), mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; copyGeneration.current++; }; }, []);
  useEffect(() => {
    let disposed = false;
    setLoading(true); setError("");
    void loadMetadata().then(snapshot => {
      if (!disposed) { setItems(snapshot.items); setVersion(snapshot.version); }
    }).catch((failure: unknown) => { if (!disposed) setError(String(failure)); }).finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [reload]);
  useEffect(() => {
    let disposed = false, unlisten: UnlistenFn | undefined;
    void getCurrentWindow().listen("help-shown", () => { input.current?.focus(); setReload(value => value + 1); })
      .then(stop => { if (disposed) stop(); else unlisten = stop; })
      .catch((failure: unknown) => { if (!disposed) setError("Could not listen for Help updates: " + String(failure)); });
    // Acknowledge the mounted view, including a recoverable metadata error state.
    void invoke("help_center_ready").catch((failure: unknown) => { if (!disposed) setError("Could not initialize Help: " + String(failure)); });
    return () => { disposed = true; unlisten?.(); };
  }, []);
  useEffect(() => { panel.current?.scrollTo({ top: 0 }); }, [query, category]);
  useEffect(() => {
    if (!feedback) return;
    const timer = window.setTimeout(() => setFeedback(null), 3200);
    return () => window.clearTimeout(timer);
  }, [feedback]);
  async function copy(text: string) {
    const generation = ++copyGeneration.current;
    setFeedback(null);
    const message = await copyHelpExample(text, value => navigator.clipboard.writeText(value));
    if (mounted.current && generation === copyGeneration.current) setFeedback({ text: message, generation });
  }
  function resetFilters() { setQuery(""); setCategory(null); input.current?.focus(); }
  const categories = helpCategories(items);
  const results = filterHelp(items, query, category);
  const groups = query.trim() ? [["Search results", results] as const] : [...groupCapabilities(results)].sort(([a], [b]) => categories.findIndex(([name]) => name === a) - categories.findIndex(([name]) => name === b));
  return <main className="help-center">
    <a className="help-skip" href="#help-reference">Skip to command reference</a>
    <header className="help-header"><div className="help-brand-row"><span className="help-mark" aria-hidden="true">N</span><span className="help-eyebrow">YOUR LOCAL COMMAND GUIDE</span><span className="help-version">{version ? `v${version}` : loading ? "Loading version…" : "Version unavailable"}</span></div>
      <h1>NEXUS Help</h1><p>Commands, shortcuts and workflows, in one place.</p>
      <div className="help-search-wrap"><label htmlFor="help-search">Search the reference</label><input id="help-search" ref={input} autoFocus type="search" placeholder="Commands, shortcuts, aliases…" value={query} onChange={event => setQuery(event.target.value)} />
        <span className="help-search-hint">Search stays on this device</span></div>
    </header>
    <div className="help-layout"><HelpNavigation categories={categories} count={items.filter(item => item.discoverable).length} selected={category} onSelect={setCategory} />
    <section className="help-reference" id="help-reference" ref={panel} tabIndex={-1} aria-label="Command reference" aria-busy={loading}>
      <div className="help-results-heading"><p role="status">{loading ? "Loading the reference…" : `${results.length} ${results.length === 1 ? "entry" : "entries"}${category ? ` in ${category}` : " in the reference"}`}</p><button type="button" onClick={() => setReload(value => value + 1)} disabled={loading}>Refresh</button></div>
      {loading ? <div className="help-state" role="status"><span className="help-loading-dot" /> Loading commands and shortcuts…</div>
        : error ? <div className="help-state help-error" role="alert"><h2>Couldn’t load the reference</h2><p>{error}</p><button type="button" onClick={() => setReload(value => value + 1)}>Try again</button></div>
        : !items.length ? <div className="help-state"><h2>No help entries yet</h2><p>The capability catalog is empty. Try refreshing the reference.</p></div>
        : !results.length ? <div className="help-state"><h2>No matching commands</h2><p>Try another name, alias or example, or explore all categories.</p><button type="button" onClick={resetFilters}>Reset filters</button></div>
        : groups.map(([name, entries]) => <section className="help-group" key={name} aria-label={name}><div className="help-group-heading"><h2>{name}</h2><span>{entries.length}</span></div>
          {name === "Getting Started" && <p className="help-group-intro">Keep your hands on the keyboard. These shortcuts are ready when you are.</p>}
          <div className="help-cards">{entries.map(item => <HelpCard key={item.id} item={item} onCopy={text => void copy(text)} />)}</div>
        </section>)}
    </section></div>
    <footer className="help-footer"><span>Local first · No account needed</span><span role="status" aria-live="polite">{feedback?.text || "Browse commands · Copy examples"}</span></footer>
  </main>;
}
