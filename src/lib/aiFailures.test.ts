import { describe, expect, it } from "vitest";
import { formatAIFailureReason } from "./aiFailures";

describe("formatAIFailureReason", () => {
  it("formats stable AI failure codes for display", () => {
    expect(
      formatAIFailureReason(
        "ai.model_auth: Model authentication failed. Check the provider API key in Settings.",
      ),
    ).toEqual({
      failureCode: "ai.model_auth",
      title: "Model authentication failed",
      code: "ERR_MODEL_AUTH",
      retryable: false,
      action: "open_settings",
      message: "Model authentication failed. Check the provider API key in Settings.",
    });
  });

  it("falls back for legacy free-text AI failures", () => {
    expect(formatAIFailureReason("model output schema invalid: missing summary")).toEqual({
      title: "AI analysis failed",
      code: "ERR_MODEL_OUTPUT_SCHEMA",
      retryable: true,
      action: "retry",
      message: "model output schema invalid: missing summary",
    });
  });
});
