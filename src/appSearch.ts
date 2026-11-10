import type { CatalogApp } from "./appCatalog.ts";
import type { Command } from "./commands.ts";

export type SearchItem = {
  readonly key: string;
  readonly label: string;
  readonly aliases: readonly string[];
} & ({ readonly kind: "app"; readonly app: CatalogApp } | { readonly kind: "command"; readonly command: Command });

export interface IndexedItem {
  readonly item: SearchItem;
  readonly name: string;
  readonly aliases: readonly string[];
  readonly words: readonly string[];
}

export type LaunchCounts = Readonly<Record<string, number>>;
export const RESULT_BATCH_SIZE = 8;

const aliasesByName: Readonly<Record<string, readonly string[]>> = {
  "visual studio code": ["vscode", "vs code"],
  "google chrome": ["chrome"],
  "microsoft edge": ["edge"],
  "calculator": ["calc"],
  "windows calculator": ["calc", "calculator"],
};

export function normalizeSearch(value: string): string {
  return value.normalize("NFKC").trim().toLowerCase().replace(/\s+/g, " ");
}

export function indexApplications(apps: readonly CatalogApp[]): readonly IndexedItem[] {
  return apps.map((app) => indexItem({ kind: "app", key: "app:" + app.id,
    label: app.display_name, aliases: aliasesByName[normalizeSearch(app.display_name)] ?? [], app }));
}

export function indexCommand(command: Command): IndexedItem {
  return indexItem({ kind: "command", key: "command:" + command.id, label: command.label, aliases: command.aliases, command });
}

function indexItem(item: SearchItem): IndexedItem {
  const name = normalizeSearch(item.label);
  return { item, name, aliases: item.aliases.map(normalizeSearch), words: name.split(/[^\p{L}\p{N}]+/u).filter(Boolean) };
}

/** Restricted Damerau-Levenshtein: insertion/deletion/substitution/transposition. */
function editDistance(left: string, right: string): number {
  let previousPrevious: number[] = [];
  let previous = Array.from({ length: right.length + 1 }, (_, i) => i);
  for (let i = 1; i <= left.length; i++) {
    const row = [i];
    for (let j = 1; j <= right.length; j++) {
      row[j] = Math.min(row[j - 1] + 1, previous[j] + 1, previous[j - 1] + (left[i - 1] === right[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && left[i - 1] === right[j - 2] && left[i - 2] === right[j - 1]) {
        row[j] = Math.min(row[j], previousPrevious[j - 2] + 1);
      }
    }
    previousPrevious = previous;
    previous = row;
  }
  return previous[right.length];
}

function match(index: IndexedItem, query: string): readonly [number, number] | null {
  if (!query || index.name === query || index.aliases.includes(query)) return [0, 0];
  const names = index.item.kind === "command" ? [index.name, ...index.aliases] : [index.name];
  if (names.some((name) => name.startsWith(query))) return [1, 0];
  const terms = query.split(" ");
  if (index.words.some((_, start) => terms.every((term, offset) => index.words[start + offset]?.startsWith(term)))) return [2, 0];
  if (names.some((name) => name.includes(query))) return [3, 0];
  // No fuzzy matching of short queries, punctuation/command strings, or long input.
  if (query.length < 4 || query.length > 64 || !/^[\p{L}\p{N}]+(?: [\p{L}\p{N}]+)*$/u.test(query)) return null;
  const limit = query.length >= 8 ? 2 : 1;
  const candidates = [index.name, ...index.aliases, ...(terms.length === 1 ? index.words : [])];
  let distance = Infinity;
  for (const candidate of candidates) {
    if (candidate.length > 64 || Math.abs(candidate.length - query.length) > limit) continue;
    const edits = editDistance(query, candidate);
    // At least 80% similarity, in addition to the hard two-edit ceiling.
    if (edits <= limit && edits / Math.max(query.length, candidate.length) <= 0.2) distance = Math.min(distance, edits);
  }
  return Number.isFinite(distance) ? [4, distance] : null;
}

export function rankApplications(index: readonly IndexedItem[], query: string, counts: LaunchCounts = {}): readonly SearchItem[] {
  const normalized = normalizeSearch(query);
  const bonus = (key: string) => {
    const count = counts[key];
    return Number.isFinite(count) && count > 0 ? Math.min(Math.log2(count + 1), 4) * 0.025 : 0;
  };
  return index.flatMap((entry) => {
    const score = match(entry, normalized);
    return score ? [{ entry, tier: score[0], distance: score[1] }] : [];
  }).sort((a, b) => a.tier - b.tier || a.distance - b.distance
    // Keep an equally matching working v0.1 command ahead of its app preview.
    || Number(a.entry.item.kind === "app") - Number(b.entry.item.kind === "app")
    || bonus(b.entry.item.key) - bonus(a.entry.item.key)
    || a.entry.name.localeCompare(b.entry.name) || a.entry.item.key.localeCompare(b.entry.item.key))
    .map(({ entry }) => entry.item);
}

export function moveSelection(index: number, direction: number, length: number): number {
  return length === 0 ? 0 : (index + direction + length) % length;
}

export function visibleResultCount(current: number, selectedIndex: number, total: number): number {
  return Math.min(total, Math.max(current, Math.ceil((selectedIndex + 1) / RESULT_BATCH_SIZE) * RESULT_BATCH_SIZE));
}
