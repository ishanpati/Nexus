import { invoke } from "@tauri-apps/api/core";
import { createInstalledAppActions } from "../installedAppActions";

export const installedAppActions = createInstalledAppActions(invoke);
