import { invoke } from "@tauri-apps/api/core";
import { toAppUiError } from "./errors";
import type { IpcResponse } from "../types/api";

export async function invokeCommand<TArgs extends object, TResult>(
  command: string,
  args: TArgs,
): Promise<TResult> {
  let response: IpcResponse<TResult>;

  try {
    response = await invoke<IpcResponse<TResult>>(command, args as Record<string, unknown>);
  } catch (error) {
    throw toAppUiError({
      code: "ERR_UNKNOWN",
      message: normalizeTransportError(error),
    });
  }

  if (response.status === "error") {
    throw toAppUiError(response.error);
  }

  if (response.data === undefined) {
    throw toAppUiError({
      code: "ERR_UNKNOWN",
      message: `Command ${command} returned no data.`,
    });
  }

  return response.data;
}

function normalizeTransportError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);

  if (message.includes("invoke") || message.includes("__TAURI_INTERNALS__")) {
    return "Tauri runtime is not available in this browser preview. Run the desktop app to use native commands.";
  }

  return message;
}
