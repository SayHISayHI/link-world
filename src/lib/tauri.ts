import { invoke } from "@tauri-apps/api/core";
import { toAppUiError } from "./errors";
import type { IpcResponse } from "../types/api";

export async function invokeCommand<TArgs extends Record<string, unknown>, TResult>(
  command: string,
  args: TArgs,
): Promise<TResult> {
  const response = await invoke<IpcResponse<TResult>>(command, args);

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
