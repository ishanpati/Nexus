import type { SearchItem } from "./appSearch.ts";
import type { CommandId } from "./commands.ts";
import type { FileResult } from "./fileSearch.ts";
import type { FileAction, FileOutcome } from "./fileActions.ts";
import { isAudioAction, audioStateMessage, audioErrorMessage, type AudioAction, type AudioState } from "./services/audioService.ts";
import type { WindowsAction, SystemOutcome } from "./services/systemActionService.ts";

export type ResultItem = SearchItem | { kind: "file"; key: string; label: string; file: Pick<FileResult, "full_path"> };
export type ExecutionOptions = { fileAction?: FileAction; confirmed?: boolean; token?: string };
type Outcome = { status: "success"; message?: string; warning?: string | null }
  | { status: "help"; message: string }
  | { status: "file_confirmation"; path: string; message: string }
  | { status: "sleep_confirmation"; token: string; message: string };
export interface Handlers {
  command(id: CommandId): Promise<void>;
  app(id: string): Promise<{ launched: boolean; warning: string | null }>;
  file(file: Pick<FileResult, "full_path">, action: FileAction, confirmed?: boolean): Promise<FileOutcome>;
  audio(action: AudioAction): Promise<AudioState>;
  system(action: WindowsAction, token?: string): Promise<SystemOutcome>;
}
/** Executes the selected immutable row, never reparses the input. Used by click and Enter. */
export async function dispatchResult(result: ResultItem, handlers: Handlers, options: ExecutionOptions = {}): Promise<Outcome> {
  switch (result.kind) {
    case "command": await handlers.command(result.command.id); return { status: "success" };
    case "app": {
      const outcome = await handlers.app(result.app.id);
      if (!outcome.launched) throw new Error("Windows did not confirm opening the application.");
      return { status: "success", warning: outcome.warning };
    }
    case "file": {
      const outcome = await handlers.file(result.file, options.fileAction ?? "open", options.confirmed);
      return outcome.status === "confirmation_required" ? { ...outcome, status: "file_confirmation" }
        : { status: "success", warning: outcome.warning };
    }
    case "system": {
      if (result.system.status !== "ready") return { status: "help", message: result.system.description };
      const action = result.system.action;
      if (isAudioAction(action)) {
        try { return { status: "success", message: audioStateMessage(await handlers.audio(action)) }; }
        catch (error) { throw new Error(audioErrorMessage(error)); }
      }
      const outcome = await handlers.system(action, options.token);
      return outcome.status === "confirmation_required" ? { ...outcome, status: "sleep_confirmation" }
        : { ...outcome, status: "success" };
    }
  }
}

interface Lifecycle {
  reset(): void;
  restore(): void;
  hide(): Promise<void>;
  error(message: string): void;
}
/** One success/failure policy, including reset before lock or confirmed sleep. */
export async function executeWithLifecycle(operation: () => Promise<Outcome>, lifecycle: Lifecycle, resetBefore = false): Promise<Outcome | undefined> {
  if (resetBefore) lifecycle.reset();
  let outcome: Outcome;
  try { outcome = await operation(); }
  catch (error) {
    if (resetBefore) lifecycle.restore();
    lifecycle.error(error && typeof error === "object" && "message" in error ? String(error.message) : String(error));
    return;
  }
  if (outcome.status === "success") {
    lifecycle.reset();
    try { await lifecycle.hide(); }
    catch (error) { lifecycle.error("Action succeeded, but NEXUS could not hide. Do not repeat it. " + String(error)); }
  }
  return outcome;
}

export function dismissLauncher(reset: () => void, hide: () => Promise<void>) { reset(); return hide(); }
export function summonLauncher(reset: () => void, focus: () => void) { reset(); focus(); }
