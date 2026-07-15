import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUiStore } from "../../store/uiStore";
import { TopBar } from "./TopBar";

const windowActions = vi.hoisted(() => ({
  close: vi.fn().mockResolvedValue(undefined),
  minimize: vi.fn().mockResolvedValue(undefined),
  toggleMaximize: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowActions,
}));

const paneWidths = {
  sidebar: 248,
  list: 372,
  detailSidebar: 320,
};

beforeEach(() => {
  window.localStorage.clear();
  vi.clearAllMocks();
  Object.defineProperty(window.navigator, "platform", {
    configurable: true,
    value: "Win32",
  });
  useUiStore.setState({ locale: "en", theme: "dark", sidebarCollapsed: false, paneWidths });
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

  it("provides a deep drag region and routes custom window controls through Tauri", async () => {
    render(
      <TopBar
        searchValue=""
        onSearchValueChange={vi.fn()}
        onClearSearch={vi.fn()}
        captureLoading={false}
        onCaptureSubmit={vi.fn()}
      />,
    );

    expect(screen.getByTestId("app-titlebar")).toHaveAttribute("data-tauri-drag-region", "deep");
    expect(screen.getByPlaceholderText("Search or paste a URL to save...").parentElement).toHaveAttribute(
      "data-tauri-drag-region",
      "false",
    );
    expect(screen.getByRole("group", { name: "Window controls" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
    fireEvent.click(screen.getByRole("button", { name: "Maximize or restore window" }));
    fireEvent.click(screen.getByRole("button", { name: "Close window" }));

    await waitFor(() => {
      expect(windowActions.minimize).toHaveBeenCalledOnce();
      expect(windowActions.toggleMaximize).toHaveBeenCalledOnce();
      expect(windowActions.close).toHaveBeenCalledOnce();
    });
  });

  it("leaves room for native macOS traffic lights without rendering custom controls", () => {
    Object.defineProperty(window.navigator, "platform", {
      configurable: true,
      value: "MacIntel",
    });

    render(
      <TopBar
        searchValue=""
        onSearchValueChange={vi.fn()}
        onClearSearch={vi.fn()}
        captureLoading={false}
        onCaptureSubmit={vi.fn()}
      />,
    );

    expect(screen.getByTestId("native-window-controls-inset")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Window controls" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Close window" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Minimize window" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Maximize or restore window" })).not.toBeInTheDocument();
    expect(windowActions.close).not.toHaveBeenCalled();
    expect(windowActions.minimize).not.toHaveBeenCalled();
    expect(windowActions.toggleMaximize).not.toHaveBeenCalled();
  });
});
