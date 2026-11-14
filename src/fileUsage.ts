// File opens are separate from application usage. Only successful opens count;
// revealing, requesting confirmation, cancellation, and failures do not.
export const FILE_USAGE_KEY = "nexus.file-opens.v1";
export const MAX_FILE_HISTORY = 256;
const key = (path: string) => path.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();

export function parseFileHistory(raw: string | null): string[] {
  try {
    const data: unknown = JSON.parse(raw ?? "[]");
    if (!Array.isArray(data)) return [];
    const unique = new Set<string>();
    return data.filter((path): path is string => {
      if (typeof path !== "string" || new TextEncoder().encode(path).length > 4096 || /[\u0000-\u001f\u007f-\u009f]/u.test(path) || !path) return false;
      const normalized = key(path);
      if (unique.has(normalized)) return false;
      unique.add(normalized); return true;
    }).slice(0, MAX_FILE_HISTORY);
  } catch { return []; }
}

export function withFileOpen(previous: readonly string[], path: string): string[] {
  return parseFileHistory(JSON.stringify([path, ...previous.filter((item) => key(item) !== key(path))]));
}

let session: string[] | undefined;
export function openedFiles(): readonly string[] {
  if (!session) {
    try { session = parseFileHistory(localStorage.getItem(FILE_USAGE_KEY)); }
    catch { session = []; }
  }
  return session;
}

export function recordFileOpen(path: string): string | null {
  session = withFileOpen(openedFiles(), path);
  try { localStorage.setItem(FILE_USAGE_KEY, JSON.stringify(session)); return null; }
  catch { return "File-open history is available for this session only."; }
}
