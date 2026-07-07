import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("renders backend navigation and creates collections", () => {
    const onNavigate = vi.fn();
    const onCreateCollection = vi.fn();
    render(
      <Sidebar
        route={{ name: "library", view: { kind: "system", id: "inbox" } }}
        navigation={{
          systemViews: [
            {
              id: "inbox",
              label: "Inbox",
              count: 3,
              kind: "system",
              iconKey: "inbox",
            },
          ],
          collections: [
            {
              id: "collection-1",
              label: "Research",
              count: 7,
              kind: "collection",
              revision: 1,
            },
          ],
          topics: [],
          smartViews: [],
        }}
        loading={false}
        mutationLoading={false}
        onNavigate={onNavigate}
        onCreateCollection={onCreateCollection}
        onCreateSmartView={vi.fn()}
        onRenameCollection={vi.fn()}
        onArchiveCollection={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: /Inbox/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByText("7")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "New collection" }));
    fireEvent.change(screen.getByLabelText("Collection name"), {
      target: { value: "Research" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create collection" }));
    expect(onCreateCollection).toHaveBeenCalledWith("Research");
  });
});
