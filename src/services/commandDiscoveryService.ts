import type { NativeDiscoveryMetadata } from "../capabilityMetadata.ts";
import { discoverCapabilities } from "../commandDiscovery.ts";

type Transport = <T>(command: string) => Promise<T>;

/** Caller supplies Tauri invoke. This service can only retrieve descriptive data. */
export const createCommandDiscoverySnapshotService = (invoke: Transport) => async () => {
  const metadata = await invoke<NativeDiscoveryMetadata>("command_discovery_metadata");
  return { items: discoverCapabilities(metadata), version: metadata.applicationVersion };
};

export const createCommandDiscoveryService = (invoke: Transport) => async () =>
  (await createCommandDiscoverySnapshotService(invoke)()).items;
