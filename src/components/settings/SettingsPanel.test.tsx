import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("fills the route content area and owns the only vertical scroll region", () => {
    const onPanelChange = vi.fn();
    render(<SettingsPanel panel="privacy" onPanelChange={onPanelChange} />);

    expect(screen.getByTestId("settings-panel")).toHaveClass("h-full", "min-h-0");
    expect(screen.getByTestId("settings-panel")).not.toHaveClass("h-screen");
    expect(screen.getByTestId("settings-scroll-region")).toHaveClass(
      "min-h-0",
      "overflow-y-auto",
    );

    fireEvent.click(screen.getByRole("button", { name: "About" }));
    expect(onPanelChange).toHaveBeenCalledWith("about");
  });
});
