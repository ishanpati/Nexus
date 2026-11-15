/** Descriptive metadata only. Rust remains authoritative; IDs are not a UI enum. */
export type ActionParameter = {
  key: string;
  label: string;
  required: boolean;
  default?: string | number | boolean;
} & (
  | { type: "text"; max_length: number }
  | { type: "number"; min: number; max: number; integer: boolean }
  | { type: "boolean" | "application" | "file" | "folder" }
  | { type: "choice"; options: readonly string[] }
);
export interface WorkflowActionDescriptor {
  id: string;
  display_name: string;
  description: string;
  category: string;
  icon: "application" | "file" | "folder" | "volume" | "settings" | "lock" | "moon";
  parameters: readonly ActionParameter[];
  workflow_compatible: boolean;
  confirmation: "none" | "sensitive_target" | "always";
  terminal: boolean;
  schema_version: number;
}
type Transport = <T>(command: string) => Promise<T>;
export const createWorkflowActionCatalog = (invoke: Transport) =>
  () => invoke<readonly WorkflowActionDescriptor[]>("list_workflow_actions");
