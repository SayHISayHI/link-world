import type { AppUiError } from "./errors";
import type { IpcErrorCode } from "../types/api";

interface AIFailureMetadata {
  title: string;
  code: IpcErrorCode;
  retryable: boolean;
  action?: AppUiError["action"];
}

export interface AIFailureDisplay extends AIFailureMetadata {
  failureCode?: string;
  message: string;
}

const AI_FAILURE_METADATA: Record<string, AIFailureMetadata> = {
  "ai.timeout": {
    title: "AI request timed out",
    code: "ERR_NETWORK_TIMEOUT",
    retryable: true,
    action: "retry",
  },
  "ai.model_auth": {
    title: "Model authentication failed",
    code: "ERR_MODEL_AUTH",
    retryable: false,
    action: "open_settings",
  },
  "ai.rate_limit": {
    title: "Model rate limit reached",
    code: "ERR_MODEL_RATE_LIMIT",
    retryable: true,
    action: "retry",
  },
  "ai.model_not_found": {
    title: "Model not found",
    code: "ERR_MODEL_NOT_FOUND",
    retryable: false,
    action: "open_settings",
  },
  "ai.output_schema": {
    title: "Invalid model response",
    code: "ERR_MODEL_OUTPUT_SCHEMA",
    retryable: true,
    action: "retry",
  },
  "ai.policy_denied": {
    title: "AI blocked by policy",
    code: "ERR_POLICY_DENIED",
    retryable: false,
    action: "open_settings",
  },
  "ai.not_configured": {
    title: "No default model configured",
    code: "ERR_POLICY_DENIED",
    retryable: false,
    action: "open_settings",
  },
  "ai.provider_config_invalid": {
    title: "Model settings incomplete",
    code: "ERR_POLICY_DENIED",
    retryable: false,
    action: "open_settings",
  },
  "ai.secret_storage": {
    title: "Model credential unavailable",
    code: "ERR_SECRET_STORAGE",
    retryable: false,
    action: "open_settings",
  },
  "ai.input_unavailable": {
    title: "Analysis input unavailable",
    code: "ERR_OBJECT_NOT_FOUND",
    retryable: false,
  },
  "ai.provider_unavailable": {
    title: "Model provider unavailable",
    code: "ERR_UNKNOWN",
    retryable: true,
    action: "retry",
  },
  "ai.local_failure": {
    title: "Local AI update failed",
    code: "ERR_UNKNOWN",
    retryable: true,
    action: "view_logs",
  },
  "ai.failed": {
    title: "AI analysis failed",
    code: "ERR_UNKNOWN",
    retryable: true,
    action: "retry",
  },
};

const FALLBACK_METADATA: AIFailureMetadata = {
  title: "AI analysis failed",
  code: "ERR_MODEL_OUTPUT_SCHEMA",
  retryable: true,
  action: "retry",
};

export function formatAIFailureReason(reason: string): AIFailureDisplay {
  const trimmed = reason.trim();
  const [maybeCode, ...messageParts] = trimmed.split(":");
  const failureCode = maybeCode.startsWith("ai.") ? maybeCode : undefined;

  if (!failureCode) {
    return {
      ...FALLBACK_METADATA,
      message: trimmed,
    };
  }

  const metadata = AI_FAILURE_METADATA[failureCode] ?? FALLBACK_METADATA;
  const message = messageParts.join(":").trim();

  return {
    ...metadata,
    failureCode,
    message: message || trimmed,
  };
}
