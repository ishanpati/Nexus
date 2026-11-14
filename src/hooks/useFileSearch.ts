import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createFileSearchStore, type FileResult } from "../fileSearch";

const store = createFileSearchStore((query, limit) => invoke<readonly FileResult[]>("search_files", { query, limit }));

export function useFileSearch(query: string | null) {
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot);
  useEffect(() => {
    store.update(query);
  }, [query]);
  useEffect(() => () => { store.update(null); }, []);
  return { ...snapshot, retry: store.retry };
}
