import { isSystemAction, type SystemAction } from "../systemCommands.ts";

export type AudioAction = Extract<SystemAction, { id: "get_audio_state" | "set_volume" | "adjust_volume" | "set_mute" }>;
export interface AudioState { readonly volume_percent: number; readonly muted: boolean; readonly endpoint_id: string }
export function isAudioAction(action: SystemAction): action is AudioAction {
  return ["get_audio_state", "set_volume", "adjust_volume", "set_mute"].includes(action.id);
}
type Transport = <T>(command: string, args: Record<string, unknown>) => Promise<T>;
export function createAudioActions(invoke: Transport) {
  return async (request: AudioAction): Promise<AudioState> => {
    if (!isSystemAction(request) || !isAudioAction(request)) throw new Error("Invalid audio command.");
    return invoke<AudioState>("execute_audio", { request });
  };
}
export function audioStateMessage(state: AudioState): string {
  return `Volume ${state.volume_percent}% · ${state.muted ? "Muted" : "Unmuted"}`;
}
export function audioErrorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
    return error.message + ("may_have_changed" in error && error.may_have_changed ? " Audio may have changed. Use audio status before retrying." : "");
  }
  return "Could not complete the audio request. " + String(error);
}
