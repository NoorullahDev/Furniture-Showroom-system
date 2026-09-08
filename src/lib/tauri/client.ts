import { invoke } from "@tauri-apps/api/core";

export type CommandError = {
  code: string;
  message: string;
  correlationId?: string;
  fieldErrors?: Record<string, string>;
};

function toCommandError(e: unknown): CommandError {
  if (typeof e === "object" && e !== null && "code" in e) {
    return e as CommandError;
  }
  return {
    code: "INTERNAL",
    message: typeof e === "string" ? e : "Unexpected error",
  };
}

export async function runCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return (await invoke<T>(command, args)) as T;
  } catch (e) {
    throw toCommandError(e);
  }
}