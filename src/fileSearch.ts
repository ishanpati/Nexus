export interface FileResult {
  readonly full_path: string;
  readonly filename: string;
  readonly extension: string;
  readonly is_directory: boolean;
}
export type SearchMode = { kind: "apps" } | { kind: "files"; query: string };
export const FILE_BATCH_SIZE = 10;
export const FILE_RESULT_LIMIT = 100;
export const FILE_DEBOUNCE_MS = 150;

export function searchMode(input: string): SearchMode {
  return /^f /i.test(input) ? { kind: "files", query: input.slice(2).trim() } : { kind: "apps" };
}

export function parentFolder(path: string): string {
  if (/^[a-z]:[\\/]$/i.test(path)) return path;
  const trimmed = path.replace(/[\\/]+$/, "");
  const separator = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  if (separator < 0) return trimmed;
  return separator === 2 && trimmed[1] === ":" ? trimmed.slice(0, 3) : trimmed.slice(0, separator);
}

/** Text segments only: never interpret filenames as markup or regex. */
export function highlightParts(text: string, query: string): readonly { text: string; match: boolean }[] {
  if (!query) return [{ text, match: false }];
  const lower = text.toLowerCase();
  const needle = query.toLowerCase();
  // Case mappings that change UTF-16 length cannot safely index the source.
  if (lower.length !== text.length || needle.length !== query.length) return [{ text, match: false }];
  const parts: { text: string; match: boolean }[] = [];
  let offset = 0;
  for (let index = lower.indexOf(needle); index >= 0; index = lower.indexOf(needle, offset)) {
    if (index > offset) parts.push({ text: text.slice(offset, index), match: false });
    offset = index + needle.length;
    parts.push({ text: text.slice(index, offset), match: true });
  }
  if (offset < text.length) parts.push({ text: text.slice(offset), match: false });
  return parts;
}

export interface FileSnapshot {
  readonly query: string | null;
  readonly resultQuery: string;
  readonly results: readonly FileResult[];
  readonly loading: boolean;
  readonly error: string;
}

export function fileSearchError(error: unknown): string {
  const details = error && typeof error === "object" ? error as { code?: unknown; message?: unknown } : {};
  if (details.code === "everything_unavailable") return "Everything is unavailable. Start the Everything desktop client and retry.";
  if (details.code === "es_not_found") return "Everything’s ES tool was not found. Configure NEXUS_ES_PATH or PATH, restart NEXUS, and retry.";
  if (details.code === "timeout") return "Everything took too long to respond. Wait for indexing to finish and retry.";
  if (details.code === "busy") return "Everything is handling another search. Please retry shortly.";
  return typeof details.message === "string" ? details.message : "Could not search files. " + String(error);
}

type Timer = (callback: () => void, delay: number) => () => void;
const schedule: Timer = (callback, delay) => { const timer = setTimeout(callback, delay); return () => clearTimeout(timer); };

/** Serialize IPC because Rust permits one ES child. Keep only the latest pending
 * query; generation checks reject stale successes AND failures across mode changes. */
export function createFileSearchStore(
  search: (query: string, limit: number) => Promise<readonly FileResult[]>,
  timer: Timer = schedule,
) {
  let snapshot: FileSnapshot = { query: null, resultQuery: "", results: [], loading: false, error: "" };
  const listeners = new Set<() => void>();
  let generation = 0;
  let active = false;
  let ready = false;
  let cancelTimer: (() => void) | undefined;
  const publish = (next: FileSnapshot) => { snapshot = next; listeners.forEach((listener) => listener()); };
  const start = () => {
    if (active || !ready || !snapshot.query) return;
    const token = generation;
    const query = snapshot.query;
    active = true;
    ready = false;
    void Promise.resolve().then(() => search(query, FILE_RESULT_LIMIT)).then((results) => {
      if (token !== generation) return;
      publish({ query, resultQuery: query, results: results.slice(0, FILE_RESULT_LIMIT), loading: false, error: "" });
    }).catch((error: unknown) => {
      if (token !== generation) return;
      publish({ query, resultQuery: "", results: [], loading: false, error: fileSearchError(error) });
    }).finally(() => { active = false; start(); });
  };
  const update = (query: string | null, force = false) => {
    if (!force && query === snapshot.query) return;
    generation++;
    ready = false;
    cancelTimer?.();
    cancelTimer = undefined;
    if (!query) {
      publish({ query, resultQuery: "", results: [], loading: false, error: "" });
      return;
    }
    if (new TextEncoder().encode(query).length > 1024 || /[\u0000-\u001f\u007f-\u009f]/u.test(query)) {
      publish({ query, resultQuery: "", results: [], loading: false, error: "Use at most 1024 UTF-8 bytes without control characters." });
      return;
    }
    publish({ ...snapshot, query, loading: true, error: "" });
    cancelTimer = timer(() => { cancelTimer = undefined; ready = true; start(); }, FILE_DEBOUNCE_MS);
  };
  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
    update,
    retry: () => update(snapshot.query, true),
  };
}
