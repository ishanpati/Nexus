import { invoke } from "@tauri-apps/api/core";
import { commandRegistry, type CommandId } from "../commands";

export async function executeCommand(id: CommandId): Promise<void> {
  if (id === "create_workflow" || id === "edit_workflow") {
    await invoke("open_workflow_studio", { route: { mode: id === "create_workflow" ? "create" : "list" } });
    return;
  }
  // Only registry-owned requests cross IPC. Search text is never an argument.
  await invoke<void>("execute_command", { request: commandRegistry[id].request });
}
