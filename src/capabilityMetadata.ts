import type { WorkflowActionDescriptor } from "./services/workflowActionService.ts";

export interface Availability {
  readonly status: "available" | "unavailable" | "unknown";
  readonly reason: string;
}

/** Data only: discovery never carries requests, handlers, or execution authority. */
export interface CapabilityRegistration {
  readonly id: string;
  readonly kind: "command" | "search_mode" | "interaction" | "shortcut" | "action";
  readonly displayName: string;
  readonly description: string;
  readonly category: string;
  readonly syntax: string;
  readonly aliases: readonly string[];
  readonly examples: readonly string[];
  readonly discoverable: boolean;
  readonly actionIds?: readonly string[];
  readonly prerequisites?: readonly string[];
  readonly parameters?: readonly { name: string; description: string }[];
  readonly launcherConfirmation?: "none" | "sensitive_target" | "always" | "per_step";
  readonly availability?: Availability;
}

export interface DiscoverableCapability extends CapabilityRegistration {
  readonly actions: readonly WorkflowActionDescriptor[];
  readonly workflowCompatible: boolean;
  readonly availability: Availability;
}

export interface NativeDiscoveryMetadata {
  readonly applicationVersion: string;
  readonly actions: readonly WorkflowActionDescriptor[];
  readonly launcherShortcut: { readonly keys: string; readonly registered: boolean; readonly warning: string | null };
  readonly fileSearch: Availability;
}

/** No fixed category enum: future registrations create their own groups. */
export function groupCapabilities(items: readonly DiscoverableCapability[]) {
  const groups = new Map<string, DiscoverableCapability[]>();
  for (const item of items) {
    const group = groups.get(item.category) ?? [];
    group.push(item);
    groups.set(item.category, group);
  }
  return groups;
}
