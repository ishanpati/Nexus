import type { FileResult } from "./fileSearch.ts";
export type FileAction = "open" | "reveal";
export type FileOutcome = { status: "confirmation_required"; path: string; message: string }
  | { status: "opened"; warning: string | null };
type Transport = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
export function createFileActions(invoke: Transport) {
  return (file: Pick<FileResult, "full_path">, action: FileAction, confirmed = false) =>
    invoke<FileOutcome>("open_file_result", { request: { path: file.full_path, action, confirmed } });
}

export function canActivateFile(loading: boolean, resultQuery: string, currentQuery: string | null): boolean {
  return !loading && !!currentQuery && resultQuery === currentQuery;
}
