import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObject, RebuildSearchIndexResponse } from "../../types/api";
import { ObjectList } from "./ObjectList";

const noop = vi.fn();

const object: KnowledgeObject = {
  id: "obj-search",
  userId: "local",
  type: "article",
  title: "Workflow Notes",
  privacyLevel: "personal",
  lifecycleStatus: "parsed",
  capturedAt: "2026-06-25T00:00:00.000Z",
  updatedAt: "2026-06-25T00:00:00.000Z",
};

function renderObjectList(
  overrides: Partial<Parameters<typeof ObjectList>[0]> = {},
) {
  return render(
    <ObjectList
      objects={[object]}
      heading="All"
      hasMore={false}
      loading={false}
      searchValue=""
      searchResults={[]}
      searchLoading={false}
      searchMaintenanceLoading={false}
      onCancelSearchIndexRebuild={noop}
      onCheckSearchIndex={noop}
      onRebuildSearchIndex={noop}
      onLoadMore={noop}
      onSelectObject={noop}
      {...overrides}
    />,
  );
}

describe("ObjectList search states", () => {
  it("keeps an empty list concise while the main panel owns onboarding", () => {
    renderObjectList({
      objects: [],
    });

    expect(screen.getByText("No items yet")).toBeInTheDocument();
    expect(screen.getByText("Use the main panel to save your first URL.")).toBeInTheDocument();
    expect(screen.queryByText("Your first useful loop")).not.toBeInTheDocument();
  });

  it("renders an actionable empty search state", () => {
    renderObjectList({
      objects: [],
      searchValue: "durable workflow",
      searchResults: [],
    });

    expect(screen.getByText("No matching objects")).toBeInTheDocument();
    expect(screen.getByText(/durable workflow/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check index" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rebuild index" })).toBeInTheDocument();
  });

  it("renders search failures with maintenance actions", () => {
    const onCheckSearchIndex = vi.fn();
    const error: AppUiError = {
      code: "ERR_PARSE_FAILED",
      title: "Search failed",
      message: "invalid search query",
      retryable: true,
    };

    renderObjectList({
      searchValue: "broken",
      searchError: error,
      onCheckSearchIndex,
    });

    expect(screen.getByText("Search failed")).toBeInTheDocument();
    expect(screen.getByText("invalid search query")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Check index" }));
    expect(onCheckSearchIndex).toHaveBeenCalledOnce();
  });

  it("renders rebuild progress and supports cancellation before finalizing", () => {
    const onCancelSearchIndexRebuild = vi.fn();

    renderObjectList({
      searchMaintenanceLoading: true,
      searchRebuildStatus: rebuildStatus({
        stage: "indexing",
        indexedObjects: 10,
        expectedObjects: 20,
        progressPercent: 50,
        cancellable: true,
      }),
      onCancelSearchIndexRebuild,
    });

    expect(screen.getByText(/Indexing searchable objects/)).toBeInTheDocument();
    expect(screen.getByText("50%")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancelSearchIndexRebuild).toHaveBeenCalledOnce();
  });

  it("explains the non-cancellable finalizing boundary", () => {
    renderObjectList({
      searchMaintenanceLoading: true,
      searchRebuildStatus: rebuildStatus({
        stage: "finalizing",
        indexedObjects: 20,
        expectedObjects: 20,
        progressPercent: 100,
        cancellable: false,
      }),
    });

    expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
    expect(screen.getByText(/Finalizing is atomic/)).toBeInTheDocument();
  });
});

function rebuildStatus(
  overrides: Partial<RebuildSearchIndexResponse>,
): RebuildSearchIndexResponse {
  return {
    jobId: "job-rebuild",
    status: "running",
    stage: "indexing",
    expectedObjects: 20,
    indexedObjects: 0,
    progressPercent: 0,
    cancellable: true,
    ...overrides,
  };
}
