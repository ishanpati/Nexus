import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { createStudioNavigation, studioError, type NavigationState, type StudioRequest, type StudioRoute } from "../studioNavigation";
import { createStudioService, type WorkflowView } from "../services/workflowStudioService";
import { WorkflowEditor, type WorkflowEditorHandle } from "./WorkflowEditor";
import { SavedWorkflows } from "./SavedWorkflows";
import "./WorkflowStudio.css";

const service = createStudioService(invoke);
export function WorkflowStudio() {
  const [route, setRoute] = useState<StudioRoute>({ mode: "list" });
  const [items, setItems] = useState<WorkflowView[]>([]);
  const [selected, setSelected] = useState<WorkflowView | null>(null);
  const [name, setName] = useState("");
  const [dirty, setDirty] = useState(false), dirtyRef = useRef(false);
  const [saving, setSaving] = useState(false), savingRef = useRef(false);
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [navigationState, setNavigationState] = useState<NavigationState>({ pending: null, busy: false, error: "" });
  const { pending, busy } = navigationState;
  const [loading, setLoading] = useState(false), [error, setError] = useState("");
  const [refresh, setRefresh] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null), editor = useRef<WorkflowEditorHandle>(null);
  const navigationRef = useRef<ReturnType<typeof createStudioNavigation> | null>(null);
  function markDirty(value: boolean) { dirtyRef.current = value; setDirty(value); }
  if (!navigationRef.current) navigationRef.current = createStudioNavigation({
    isDirty: () => dirtyRef.current,
    save: () => editor.current?.save() ?? Promise.resolve(false),
    commit: (request, accept) => invoke("studio_resolve", { sequence: request.sequence, accept }),
    accepted: request => {
      markDirty(false); setName(""); setSelected(null); setEditorGeneration(n => n + 1); setError("");
      setRoute(request.intent.kind === "navigate" ? request.intent.route : { mode: "list" });
      setRefresh(value => value + 1);
    },
    changed: setNavigationState,
  });
  const navigation = navigationRef.current;

  useEffect(() => {
    let disposed = false;
    const listeners: UnlistenFn[] = [];
    const appWindow = getCurrentWindow();
    const fetchPending = async () => {
      try {
        const request = await invoke<StudioRequest | null>("studio_pending");
        if (!disposed) navigation.receive(request);
      } catch (error) { if (!disposed) setError(studioError(error)); }
    };
    const track = async (promise: Promise<UnlistenFn>) => {
      const unlisten = await promise;
      if (disposed) unlisten(); else listeners.push(unlisten);
    };
    void (async () => {
      try {
        await track(appWindow.listen("studio-request", () => { void fetchPending(); }));
        await track(appWindow.listen("workflows-changed", () => { if (!disposed) setRefresh(value => value + 1); }));
        await track(appWindow.onFocusChanged(({ payload }) => {
          if (payload && !disposed) { void fetchPending(); setRefresh(value => value + 1); }
        }));
        if (!disposed) {
          const request = await invoke<StudioRequest | null>("studio_ready");
          if (!disposed) navigation.receive(request);
        }
      } catch (error) { if (!disposed) setError(studioError(error)); }
    })();
    return () => { disposed = true; listeners.forEach(unlisten => unlisten()); };
  }, [navigation]);

  useEffect(() => {
    if (pending) dialog.current?.showModal(); else dialog.current?.close();
  }, [pending]);

  useEffect(() => {
    let disposed = false;
    // Notifications never replace an in-progress editor or delete operation.
    if (dirtyRef.current || savingRef.current) { setLoading(false); return; }
    setLoading(true); setError("");
    const load = async () => {
      try {
        if (route.mode === "list") {
          const rows = await service.list();
          if (!disposed && !savingRef.current) setItems(rows);
        } else if (route.mode === "edit") {
          const view = await service.get(route.id);
          if (!disposed && !dirtyRef.current && !savingRef.current) {
            setSelected(previous => previous?.workflow.id === view.workflow.id && previous.workflow.revision > view.workflow.revision ? previous : view);
            setName(view.workflow.name);
          }
        }
      } catch (error) { if (!disposed) setError(studioError(error)); }
      finally { if (!disposed) setLoading(false); }
    };
    void load();
    return () => { disposed = true; };
  }, [route, refresh]);

  async function navigate(next: StudioRoute) {
    try { await service.open(next); }
    catch (error) { setError(studioError(error)); }
  }
  function trackSaving(value: boolean) {
    savingRef.current = value; setSaving(value); navigation.setSaving(value);
    if (!value) setRefresh(n => n + 1);
  }
  function saved(view: WorkflowView) {
    setSelected(view);
    setItems(previous => [...previous.filter(v => v.workflow.id !== view.workflow.id), view]);
  }
  const blocked = busy || saving || !!pending;
  return <main className="studio">
    <header><div><span className="studio-brand">NEXUS / LOCAL WORKFLOWS</span><h1>Workflow Studio</h1></div>
      <span>{dirty ? "Unsaved changes" : "Local workspace"}</span></header>
    <nav aria-label="Workflow Studio"><button disabled={blocked} onClick={() => void navigate({ mode: "create" })}>Create workflow</button>
      <button disabled={blocked} onClick={() => void navigate({ mode: "list" })}>Saved workflows</button></nav>
    <section aria-busy={loading}>
      {route.mode === "list" ? <SavedWorkflows items={items} loading={loading} error={error} disabled={blocked} service={service}
        onOpen={id => void navigate({ mode: "edit", id })} onCreate={() => void navigate({ mode: "create" })}
        onRefresh={() => setRefresh(n => n + 1)} onDeleting={trackSaving}
        onDeleted={id => { setItems(previous => previous.filter(v => v.workflow.id !== id)); setRefresh(n => n + 1); }} /> : <>
        <h2>{route.mode === "create" ? "Create workflow" : "Edit workflow"}</h2>
        <p>Choose and configure actions in the order they should run.</p>
        {(route.mode === "create" || selected) && <WorkflowEditor ref={editor} key={`${editorGeneration}:${route.mode === "edit" ? selected?.workflow.id : "new"}`}
          initial={selected ?? undefined} disabled={busy || !!pending || (loading && !selected)} service={service}
          onDirty={markDirty} onSaving={trackSaving} onSaved={saved} onName={setName}
          onReload={() => selected && void navigate({ mode: "edit", id: selected.workflow.id })} />}
      </>}
      {loading && <p role="status">Loading workflows…</p>}
      {error && <p role="alert" className="studio-error">{error} <button onClick={() => setRefresh(value => value + 1)}>Retry loading</button></p>}
      {!pending && navigationState.error && <p role="alert" className="studio-error">{navigationState.error}</p>}
    </section>
    <footer>Ctrl+Shift+Space opens the command bar. Closing Studio keeps NEXUS running.</footer>
    <dialog ref={dialog} onCancel={event => { event.preventDefault(); void navigation.choose("cancel"); }} aria-labelledby="unsaved-title">
      <h2 id="unsaved-title">Save unsaved changes?</h2><p>“{name || "Untitled workflow"}” has unsaved changes.</p>
      <p>{pending?.intent.kind === "quit" ? "Save or discard your changes before quitting NEXUS." : pending?.intent.kind === "close" ? "Save or discard your changes before closing Studio." : "Save or discard your changes before opening the requested view."}</p>
      <div><button autoFocus disabled={busy || saving} onClick={() => void navigation.choose("cancel")}>Cancel</button>
        <button disabled={busy || saving} onClick={() => void navigation.choose("discard")}>Discard changes</button>
        <button disabled={busy || saving} onClick={() => void navigation.choose("save")}>{saving ? "Saving…" : "Save changes"}</button></div>
      {navigationState.error && <p role="alert" className="studio-error">{navigationState.error}</p>}
    </dialog>
  </main>;
}
