export type StudioRoute = { mode: "create" } | { mode: "list" } | { mode: "edit"; id: string };
export type StudioRequest = { sequence: number; intent: { kind: "navigate"; route: StudioRoute } | { kind: "close" | "quit" } };

/** Local drafts belong to this webview. Every route change, including same-route
 * create requests, must pass the same guard as native close. */
export function needsDiscardConfirmation(dirty: boolean, request: StudioRequest): boolean {
  return dirty && ["close", "quit", "navigate"].includes(request.intent.kind);
}
export function studioError(error: unknown): string {
  return error && typeof error === "object" && "message" in error ? String(error.message) : String(error);
}

export type StudioChoice = "save" | "discard" | "cancel";
export interface NavigationState { pending: StudioRequest | null; busy: boolean; error: string }

/** Retained Rust requests and editor saves share one sequencing boundary. */
export function createStudioNavigation(options: {
  isDirty: () => boolean;
  save: () => Promise<boolean>;
  commit: (request: StudioRequest, accept: boolean) => Promise<void>;
  accepted: (request: StudioRequest) => void;
  changed: (state: NavigationState) => void;
}) {
  let current: StudioRequest | null = null, queued: StudioRequest | null = null;
  let lastResolved = 0, saving = false, resolving = false;
  let state: NavigationState = { pending: null, busy: false, error: "" };
  const publish = (next: Partial<NavigationState>) => { state = { ...state, ...next }; options.changed(state); };
  const drain = () => {
    if (!resolving && !saving && queued) {
      const request = queued; queued = null; receive(request);
    }
  };
  async function choose(choice: StudioChoice) {
    if (!current || resolving || saving) return;
    const request = current;
    resolving = true; publish({ busy: true, error: "" });
    try {
      if (choice === "save" && !await options.save()) {
        publish({ pending: request, error: "Could not save. Cancel to review the editor errors and repair the workflow, or discard your changes." });
        return;
      }
      const accept = choice !== "cancel";
      await options.commit(request, accept);
      lastResolved = request.sequence; current = null;
      publish({ pending: null });
      if (accept) options.accepted(request);
    } catch (error) { publish({ pending: request, error: studioError(error) }); }
    finally { resolving = false; publish({ busy: false }); drain(); }
  }
  function receive(request: StudioRequest | null) {
    if (!request || request.sequence <= lastResolved || current?.sequence === request.sequence) return;
    if (resolving || saving) { queued = request; return; }
    current = request;
    if (needsDiscardConfirmation(options.isDirty(), request)) publish({ pending: request, error: "" });
    else void choose("discard");
  }
  return { receive, choose, setSaving(value: boolean) { saving = value; drain(); } };
}
