import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OrganizationPanel } from "./OrganizationPanel";

describe("OrganizationPanel", () => {
  it("manages triage, collection membership, user topics, and AI suggestions", () => {
    const onMarkFiled = vi.fn();
    const onToggleCollection = vi.fn();
    const onAddTag = vi.fn();
    const onRemoveTag = vi.fn();
    const onAcceptSuggestion = vi.fn();
    const onRejectSuggestion = vi.fn();

    render(
      <OrganizationPanel
        organization={{
          objectId: "object-1",
          triageStatus: "inbox",
          collections: [
            {
              id: "collection-1",
              name: "Research",
              collectionType: "manual",
              sortOrder: 0,
              isPinned: false,
              revision: 1,
            },
          ],
          tags: [
            {
              id: "tag-1",
              name: "Rust",
              normalizedName: "rust",
              source: "user",
            },
          ],
          tagSuggestions: [
            {
              id: "suggestion-1",
              objectId: "object-1",
              analysisId: "analysis-1",
              name: "Local AI",
              normalizedName: "local ai",
              confidence: 0.9,
              rationale: "The article focuses on local model workflows.",
              status: "pending",
              createdAt: "2026-07-07T00:00:00Z",
            },
          ],
        }}
        collections={[
          {
            id: "collection-1",
            label: "Research",
            count: 1,
            kind: "collection",
          },
          {
            id: "collection-2",
            label: "Ideas",
            count: 0,
            kind: "collection",
          },
        ]}
        loading={false}
        mutationLoading={false}
        onMarkFiled={onMarkFiled}
        onToggleCollection={onToggleCollection}
        onAddTag={onAddTag}
        onRemoveTag={onRemoveTag}
        onAcceptSuggestion={onAcceptSuggestion}
        onRejectSuggestion={onRejectSuggestion}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "File" }));
    expect(onMarkFiled).toHaveBeenCalledWith(true);

    fireEvent.click(screen.getByRole("checkbox", { name: "Ideas" }));
    expect(onToggleCollection).toHaveBeenCalledWith("collection-2", true);

    fireEvent.change(screen.getByRole("textbox", { name: "Add topic" }), { target: { value: "Tauri" } });
    fireEvent.click(screen.getByRole("button", { name: "Add topic" }));
    expect(onAddTag).toHaveBeenCalledWith("Tauri");

    fireEvent.click(screen.getByRole("button", { name: "Remove Rust" }));
    expect(onRemoveTag).toHaveBeenCalledWith("tag-1");
    fireEvent.click(screen.getByRole("button", { name: "Accept Local AI" }));
    expect(onAcceptSuggestion).toHaveBeenCalledWith("suggestion-1");
    fireEvent.click(screen.getByRole("button", { name: "Reject Local AI" }));
    expect(onRejectSuggestion).toHaveBeenCalledWith("suggestion-1");
  });
});
