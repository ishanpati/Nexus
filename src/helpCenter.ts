import type { DiscoverableCapability } from "./capabilityMetadata.ts";
import type { ActionParameter } from "./services/workflowActionService.ts";

const normalized = (text: string) => text.toLowerCase().trim().replace(/\s+/g, " ");

export function helpCategories(items: readonly DiscoverableCapability[]) {
  const counts = new Map<string, number>();
  for (const item of items.filter(item => item.discoverable)) counts.set(item.category, (counts.get(item.category) ?? 0) + 1);
  return [...counts].sort(([a], [b]) => a === b ? 0 : a === "Getting Started" ? -1 : b === "Getting Started" ? 1 : a.localeCompare(b));
}

export function filterHelp(items: readonly DiscoverableCapability[], search: string, category: string | null = null) {
  const query = normalized(search);
  return items.filter(item => item.discoverable && (!category || item.category === category)).map((item, index) => {
    const name = normalized(item.displayName), syntax = normalized(item.syntax);
    const aliases = (item.aliases ?? []).map(normalized);
    const fields = [item.description, item.category, ...(item.examples ?? []), ...aliases];
    const rank = !query ? 0 : name === query ? 0 : syntax === query ? 1 : name.startsWith(query) ? 2
      : syntax.startsWith(query) ? 3 : name.includes(query) || aliases.some(alias => alias.includes(query)) ? 4
      : syntax.includes(query) ? 5 : fields.some(field => normalized(field).includes(query)) ? 6 : -1;
    return { item, rank, index };
  }).filter(result => result.rank >= 0).sort((a, b) => a.rank - b.rank || a.index - b.index).map(result => result.item);
}

/** Display existing schema constraints, without introducing a validation policy. */
export function describeParameter(parameter: ActionParameter): string {
  let text: string;
  switch (parameter.type) {
    case "number": text = `${parameter.integer ? "Whole number" : "Number"} from ${parameter.min} to ${parameter.max}`; break;
    case "text": text = `Text, up to ${parameter.max_length} characters`; break;
    case "choice": text = `Choose ${parameter.options.join(", ")}`; break;
    case "boolean": text = "Yes or no"; break;
    case "application": text = "Select an installed application"; break;
    case "file": text = "Select a file"; break;
    case "folder": text = "Select a folder"; break;
    default: text = "See the action's field instructions";
  }
  return `${text}. ${parameter.required ? "Required" : "Optional"}.${parameter.default === undefined ? "" : ` Default: ${String(parameter.default)}.`}`;
}

/** Only a write function is accepted: copying cannot invoke an executor or read clipboard data. */
export async function copyHelpExample(text: string, write: (text: string) => Promise<void>): Promise<string> {
  try { await write(text); return "Copied example."; }
  catch { return "Could not copy. Select the example text and press Ctrl+C."; }
}
