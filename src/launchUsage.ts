import type { LaunchCounts } from "./appSearch.ts";

export const USAGE_STORAGE_KEY = "nexus.launch-counts.v1";

export function parseLaunchCounts(json: string | null): LaunchCounts {
  try {
    const value: unknown = JSON.parse(json ?? "{}");
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    return Object.fromEntries(Object.entries(value).filter(([key, count]) =>
      (key.startsWith("app:") || key.startsWith("command:")) && Number.isSafeInteger(count) && count > 0,
    ).map(([key, count]) => [key, Math.min(count as number, 1_000_000)]));
  } catch { return {}; }
}

/** Call only after a confirmed successful launch, never on selection/preview. */
export function recordSuccessfulLaunch(counts: LaunchCounts, key: string): LaunchCounts {
  return { ...counts, [key]: Math.min((counts[key] ?? 0) + 1, 1_000_000) };
}
