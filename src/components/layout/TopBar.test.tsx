import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUiStore } from "../../store/uiStore";
import { TopBar } from "./TopBar";

const paneWidths = {
  sidebar: 248,
  list: 372,
  detailSidebar: 320,
};

beforeEach(() => {
  window.localStorage.clear();
  useUiStore.setState({ sidebarCollapsed: false, paneWidths });
});

describe("TopBar", () => {
  it("keeps the full brand visible when the sidebar collapses", () => {
    render(
      <TopBar
        searchValue=""
        onSearchValueChange={vi.fn()}
        onClearSearch={vi.fn()}
        captureLoading={false}
        onCaptureSubmit={vi.fn()}
      />,
    );

    const brandSlot = screen.getByTestId("topbar-sidebar-slot");
    expect(brandSlot).toHaveStyle({ width: "248px" });
    expect(screen.getByRole("heading", { name: "拾海 · Node Tide" })).toHaveTextContent("拾海 · Node Tide");

    act(() => useUiStore.getState().setSidebarCollapsed(true));

    expect(brandSlot).toHaveStyle({ width: "248px" });
    expect(screen.getByRole("heading", { name: "拾海 · Node Tide" })).toHaveTextContent("拾海 · Node Tide");
  });

  it("searches text, captures URLs, and clears the omnibox", () => {
    const onSearchValueChange = vi.fn();
    const onCaptureSubmit = vi.fn();
    const onClearSearch = vi.fn();

    function Harness() {
      const [searchValue, setSearchValue] = useState("");
      return (
        <TopBar
          searchValue={searchValue}
          onSearchValueChange={(value) => {
            onSearchValueChange(value);
            setSearchValue(value);
          }}
          onClearSearch={() => {
            onClearSearch();
            setSearchValue("");
          }}
          captureLoading={false}
          onCaptureSubmit={onCaptureSubmit}
        />
      );
    }

    render(<Harness />);
    const input = screen.getByPlaceholderText("Search or paste a URL to save...");

    fireEvent.change(input, { target: { value: "local knowledge" } });
    expect(onSearchValueChange).toHaveBeenLastCalledWith("local knowledge");

    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(onClearSearch).toHaveBeenCalledOnce();
    expect(input).toHaveValue("");

    fireEvent.change(input, { target: { value: " https://example.com/article " } });
    expect(onSearchValueChange).toHaveBeenLastCalledWith("");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCaptureSubmit).toHaveBeenCalledWith("https://example.com/article");
  });
});
