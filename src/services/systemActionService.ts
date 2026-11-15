import { isSystemAction, type SystemAction } from "../systemCommands.ts";
export type WindowsAction = Extract<SystemAction, { id: "open_settings" | "lock" | "sleep" }>;
export type SystemOutcome = { status: "confirmation_required"; token: string; message: string }
  | { status: "accepted"; message: string; warning: string | null };
type Transport = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
export function isWindowsAction(action: SystemAction): action is WindowsAction {
  return action.id === "open_settings" || action.id === "lock" || action.id === "sleep";
}
export function createSystemActions(invoke: Transport) {
  return {
    async execute(request: WindowsAction, confirmationToken?: string): Promise<SystemOutcome> {
      if (!isSystemAction(request) || !isWindowsAction(request) || (confirmationToken !== undefined && request.id !== "sleep")) throw new Error("Invalid Windows system action.");
      return invoke<SystemOutcome>("execute_system_action", { request, confirmationToken: confirmationToken ?? null });
    },
    cancel: (token: string) => invoke<void>("cancel_system_confirmation", { token }),
  };
}
export function systemActionError(error: unknown): string {
  return error && typeof error === "object" && "message" in error && typeof error.message === "string"
    ? error.message : "Could not complete the Windows action. " + String(error);
}
/** A dialog's confirm and cancel callbacks share this gate, including double clicks. */
export function confirmationGate() {
  let used = false;
  return (action: () => void) => { if (used) return false; used = true; action(); return true; };
}
