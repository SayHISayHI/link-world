import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { KnowledgeObject, KnowledgeObjectDetail } from "../../types/api";
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
  it("uses the main detail pane for first-run onboarding", () => {
    const onFocusCapture = vi.fn();
    const onOpenModelSettings = vi.fn();

    render(
      <ObjectDetail
        detailLoading={false}
        pingLoading={false}
        deleteLoading={false}
        retryLoading={false}
        aiRunLoading={false}
        searchIndexLoading={false}
        evaluationLoading={false}
        libraryEmpty
        onPing={noop}
        onDeleteObject={noop}
        onRetryCapture={noop}
        onOpenModelSettings={onOpenModelSettings}
        onFocusCapture={onFocusCapture}
        onRunAIAnalysis={noop}
        onReindexObject={noop}
        onRunEvaluation={noop}
      />,
    );

    expect(screen.getByText("Start by saving one useful link.")).toBeInTheDocument();
    expect(screen.getByText("Save your first URL")).toBeInTheDocument();
    expect(screen.getByText("Find it again")).toBeInTheDocument();
    expect(screen.getByText("Evaluate when ready")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Paste a URL" }));
    expect(onFocusCapture).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: "Configure AI (optional)" }));
    expect(onOpenModelSettings).toHaveBeenCalledOnce();
  });

  it("keeps the empty state sized to the parent pane", () => {
    const { container } = render(
      <ObjectDetail
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

    expect(screen.getByText("Select an item to inspect.")).toBeInTheDocument();
    expect(container.firstChild).toHaveClass("h-full");
    expect(container.firstChild).not.toHaveClass("h-screen");
  });

  it("prefers the latest object fields from detail data", () => {
    const object: KnowledgeObject = {
      id: "obj-latest",
      userId: "local",
      type: "article",
      title: undefined,
      canonicalUrl: "https://example.com/original",
      privacyLevel: "personal",
      lifecycleStatus: "captured",
      capturedAt: "2026-06-25T00:00:00.000Z",
      updatedAt: "2026-06-25T00:00:00.000Z",
    };

    const detail: KnowledgeObjectDetail = {
      object: {
        ...object,
        title: "Parsed article title",
        lifecycleStatus: "parsed",
      },
      snapshots: [],
      aiAnalyses: [],
      evaluations: [],
    };

    render(
      <ObjectDetail
        object={object}
        detail={detail}
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

    expect(screen.getByText("Parsed article title")).toBeInTheDocument();
    expect(screen.getByText(/article \/ parsed/i)).toBeInTheDocument();
  });

  it("ignores detail data that belongs to the previously selected object", () => {
    const object: KnowledgeObject = {
      id: "obj-current",
      userId: "local",
      type: "article",
      title: "Current article",
      canonicalUrl: "https://example.com/current",
      privacyLevel: "personal",
      lifecycleStatus: "captured",
      capturedAt: "2026-06-25T00:00:00.000Z",
      updatedAt: "2026-06-25T00:00:00.000Z",
    };
    const staleDetail: KnowledgeObjectDetail = {
      object: {
        ...object,
        id: "obj-previous",
        title: "Previous article",
        lifecycleStatus: "parsed",
      },
      snapshots: [],
      aiAnalyses: [],
      evaluations: [],
    };

    render(
      <ObjectDetail
        object={object}
        detail={staleDetail}
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

    expect(screen.getByText("Current article")).toBeInTheDocument();
    expect(screen.getByText(/article \/ captured/i)).toBeInTheDocument();
    expect(screen.queryByText("Previous article")).not.toBeInTheDocument();
  });

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
