import { describe, expect, it } from "vitest";
import { formatCaptureFailureReason } from "./captureFailures";

describe("formatCaptureFailureReason", () => {
  it("formats stable capture failure codes for display", () => {
    expect(
      formatCaptureFailureReason(
        "capture.http_forbidden: The server returned HTTP 403. Use the browser extension.",
      ),
    ).toEqual({
      code: "capture.http_forbidden",
      title: "Browser access required",
      message: "The server returned HTTP 403. Use the browser extension.",
    });
  });

  it("falls back for legacy free-text failure reasons", () => {
    expect(formatCaptureFailureReason("parse failed: old error")).toEqual({
      title: "Capture failed",
      message: "parse failed: old error",
    });
  });
});
