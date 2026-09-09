import { invoke } from "@tauri-apps/api/core";

export type CommandError = {
  code: string;
  message: string;
  correlationId?: string;
  retryAfterSecs?: number;
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

/** Normalize an unknown thrown value (e.g. from a TanStack Query error) into a CommandError. */
export function asCommandError(e: unknown): CommandError {
  return toCommandError(e);
}

/** Human-readable message from a thrown command error. */
export function commandErrorMessage(e: unknown): string {
  return toCommandError(e).message || "Unexpected error";
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