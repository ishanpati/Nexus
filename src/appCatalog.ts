/** Read-only projection of the Rust catalog. Launch metadata stays in Rust. */
export interface CatalogApp {
  readonly id: string;
  readonly display_name: string;
  readonly launch: { readonly method: "shortcut" | "packaged_app" | "shell_app" };
  readonly sources: readonly string[];
}

export interface AppCatalog {
  readonly apps: readonly CatalogApp[];
  readonly generation: number;
  readonly warnings: readonly string[];
  readonly sources: readonly { readonly completed: boolean }[];
}

export interface CatalogSnapshot {
  readonly catalog: AppCatalog | null;
  readonly loading: boolean;
  readonly error: string;
}

/** One request shared across mounts; refresh retains the last usable snapshot. */
export function createCatalogStore(fetchCatalog: (refresh: boolean) => Promise<AppCatalog>) {
  let snapshot: CatalogSnapshot = { catalog: null, loading: true, error: "" };
  let pending: Promise<void> | null = null;
  const listeners = new Set<() => void>();
  const publish = (next: CatalogSnapshot) => {
    snapshot = next;
    listeners.forEach((listener) => listener());
  };
  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    load(refresh = false): Promise<void> {
      if (pending) return pending;
      if (snapshot.catalog && !refresh) return Promise.resolve();
      publish({ ...snapshot, loading: true, error: "" });
      pending = Promise.resolve().then(() => fetchCatalog(refresh)).then((catalog) => {
        publish({ catalog, loading: false, error: "" });
      }).catch((error: unknown) => {
        publish({ ...snapshot, loading: false, error: "Could not load applications: " + String(error) });
      }).finally(() => { pending = null; });
      return pending;
    },
  };
}
