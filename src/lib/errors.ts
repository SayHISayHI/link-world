import type { IpcError, IpcErrorCode } from "../types/api";

export interface AppUiError {
  code: IpcErrorCode;
  title: string;
  message: string;
  retryable: boolean;
  action?: "retry" | "open_settings" | "use_browser_capture" | "view_logs";
}

export function toAppUiError(error?: IpcError): AppUiError {
  if (!error) {
    return {
      code: "ERR_UNKNOWN",
      title: "Request failed",
      message: "The command failed without a structured error.",
      retryable: false,
    };
  }

  switch (error.code) {
    case "ERR_NETWORK_TIMEOUT":
      return {
        code: error.code,
        title: "Network timeout",
        message: error.message,
        retryable: true,
        action: "retry",
      };
    case "ERR_PARSE_FAILED":
      return {
        code: error.code,
        title: "Parse failed",
        message: error.message,
        retryable: true,
        action: "use_browser_capture",
      };
    case "ERR_MODEL_AUTH":
      return {
        code: error.code,
        title: "Model authentication failed",
        message: error.message,
        retryable: false,
        action: "open_settings",
      };
    default:
      return {
        code: error.code,
        title: "Command failed",
        message: error.message,
        retryable: false,
        action: "view_logs",
      };
  }
}

