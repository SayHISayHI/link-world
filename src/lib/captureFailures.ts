export interface CaptureFailureDisplay {
  code?: string;
  title: string;
  message: string;
}

const CAPTURE_FAILURE_TITLES: Record<string, string> = {
  "capture.timeout": "Capture timed out",
  "capture.network_unreachable": "Network unavailable",
  "capture.http_forbidden": "Browser access required",
  "capture.http_not_found": "Source URL not found",
  "capture.http_retryable": "Source asked us to retry later",
  "capture.http_server_error": "Source server failed",
  "capture.http_error": "Source returned an error",
  "capture.restricted_page": "Browser verification required",
  "capture.unsupported_scheme": "Unsupported URL type",
  "capture.too_large": "Page too large",
  "capture.invalid_response": "Invalid server response",
  "capture.no_readable_text": "No readable content found",
  "capture.parse_failed": "Could not parse page",
  "capture.policy_denied": "Capture blocked by policy",
  "capture.failed": "Capture failed",
};

export function formatCaptureFailureReason(reason: string): CaptureFailureDisplay {
  const trimmed = reason.trim();
  const [maybeCode, ...messageParts] = trimmed.split(":");
  const code = maybeCode.startsWith("capture.") ? maybeCode : undefined;

  if (!code) {
    return {
      title: "Capture failed",
      message: trimmed,
    };
  }

  const message = messageParts.join(":").trim();

  return {
    code,
    title: CAPTURE_FAILURE_TITLES[code] ?? "Capture failed",
    message: message || trimmed,
  };
}
