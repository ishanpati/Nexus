/** Each registered ID has an explicit argument type; Notepad accepts none. */
export interface CommandArguments {
  open_notepad: Record<string, never>;
}

export type CommandId = keyof CommandArguments;
export type CommandRequest = {
  [Id in CommandId]: { readonly id: Id; readonly args: CommandArguments[Id] };
}[CommandId];

export interface Command<Id extends CommandId = CommandId> {
  readonly id: Id;
  readonly label: string;
  readonly description: string;
  readonly aliases: readonly string[];
  readonly request: Extract<CommandRequest, { id: Id }>;
}

export const commandRegistry = {
  open_notepad: {
    id: "open_notepad",
    label: "Open Notepad",
    description: "A fresh page for your thoughts",
    aliases: ["notepad", "launch notepad"],
    request: { id: "open_notepad", args: {} },
  },
} as const satisfies { [Id in CommandId]: Command<Id> };

export function findCommands(query: string): readonly Command[] {
  const normalized = query.trim().toLowerCase().replace(/\s+/g, " ");
  return Object.values(commandRegistry).filter((command) =>
    [command.label, ...command.aliases].some((name) => name.toLowerCase().includes(normalized)),
  );
}
