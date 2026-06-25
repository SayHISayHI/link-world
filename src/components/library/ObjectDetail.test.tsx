import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { KnowledgeObject } from "../../types/api";
import { ObjectDetail } from "./ObjectDetail";

const noop = vi.fn();

function renderObjectDetail(object: KnowledgeObject) {
  return render(
    <ObjectDetail
      object={object}
      detailLoading={false}
      pingLoading={false}
      deleteLoading={false}
      retryLoading={false}
      aiRunLoading={false}
      searchIndexLoading={false}
      evaluationLoading={false}
      onPing={noop}
      onDeleteObject={noop}
      onRetryCapture={noop}
      onOpenModelSettings={noop}
      onRunAIAnalysis={noop}
      onReindexObject={noop}
      onRunEvaluation={noop}
    />,
  );
}

describe("ObjectDetail", () => {
  it("formats capture failure codes as recovery-oriented detail copy", () => {
    renderObjectDetail({
      id: "obj_failed",
      userId: "local",
      type: "article",
      title: "Private article",
      canonicalUrl: "https://example.com/private",
      privacyLevel: "personal",
      lifecycleStatus: "failed",
      failureReason:
        "capture.http_forbidden: The server returned HTTP 403. Open it in your browser and save it with the browser extension.",
      capturedAt: "2026-06-25T00:00:00.000Z",
      updatedAt: "2026-06-25T00:00:00.000Z",
    });

    expect(screen.getByText("Browser access required")).toBeInTheDocument();
    expect(screen.getByText(/browser extension/)).toBeInTheDocument();
    expect(screen.queryByText(/capture.http_forbidden/)).not.toBeInTheDocument();
  });
});
