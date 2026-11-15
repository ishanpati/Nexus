import { findCommands } from "./commands.ts";
import { indexCommand, indexSystemCommand, rankApplications, type IndexedItem, type LaunchCounts } from "./appSearch.ts";
import { searchMode } from "./fileSearch.ts";
import { suggestSystemCommands } from "./systemCommands.ts";

/** One entry point for non-file results, reusing the existing ranking algorithm. */
export function searchCommandsAndApps(query: string, apps: readonly IndexedItem[], counts: LaunchCounts = {}) {
  if (searchMode(query).kind === "files") return [];
  return rankApplications([
    ...findCommands(query).map(indexCommand),
    ...suggestSystemCommands(query).map(indexSystemCommand), ...apps,
  ], query, counts);
}
