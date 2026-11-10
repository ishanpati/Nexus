import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createCatalogStore, type AppCatalog } from "../appCatalog";

const store = createCatalogStore((refresh) => invoke<AppCatalog>(
  refresh ? "refresh_installed_apps" : "list_installed_apps",
));
const refreshCatalog = () => store.load(true);

export function useAppCatalog() {
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot);
  useEffect(() => { void store.load(); }, []);
  return { ...snapshot, refreshCatalog };
}
