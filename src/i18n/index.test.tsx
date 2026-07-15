import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProviders } from "../app/providers";
import { TopBar } from "../components/layout/TopBar";
import { useUiStore } from "../store/uiStore";
import { translate } from ".";

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.className = "";
  document.documentElement.removeAttribute("data-theme");
  useUiStore.setState({ locale: "en", theme: "light" });
});


afterEach(() => {
  useUiStore.setState({ locale: "en", theme: "light" });
  document.documentElement.className = "";
  document.documentElement.removeAttribute("data-theme");
});

describe("UI localization and theme preferences", () => {
  it("translates messages with variables and falls back to the source text", () => {
    expect(translate("zh-CN", "Snapshots {count}", { count: 3 })).toBe("快照 3");
    expect(translate("zh-CN", "User-authored title")).toBe("User-authored title");
    expect(translate("en", "Snapshots {count}", { count: 3 })).toBe("Snapshots 3");
  });

  it("switches language and theme immediately and persists both choices", () => {
    render(
      <AppProviders>
        <TopBar
          searchValue=""
          onSearchValueChange={vi.fn()}
          onClearSearch={vi.fn()}
          captureLoading={false}
          onCaptureSubmit={vi.fn()}
        />
      </AppProviders>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Switch to Chinese" }));
    expect(screen.getByPlaceholderText("搜索，或粘贴网址以保存…")).toBeInTheDocument();
    expect(document.documentElement).toHaveAttribute("lang", "zh-CN");

    fireEvent.click(screen.getByRole("button", { name: "切换到深色模式" }));
    expect(document.documentElement).toHaveClass("dark");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");

    const persisted = JSON.parse(window.localStorage.getItem("link-world-ui") ?? "{}") as {
      state?: { locale?: string; theme?: string };
    };
    expect(persisted.state).toMatchObject({ locale: "zh-CN", theme: "dark" });
  });
});
