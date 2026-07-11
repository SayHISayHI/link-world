import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "../../store/uiStore";
import { SettingsRouteLayout } from "./SettingsRouteLayout";
import { ThreePaneLayout } from "./ThreePaneLayout";

const paneWidths = {
  sidebar: 248,
  list: 372,
  detailSidebar: 320,
};

beforeEach(() => {
  window.localStorage.clear();
  useUiStore.setState({
    sidebarCollapsed: false,
    detailPaneCollapsed: false,
    paneWidths,
  });
});

describe("shell layouts", () => {
  it("keeps the three-pane sidebar width in sync with its collapsed state", () => {
    render(
      <ThreePaneLayout
        topBar={<div>Top bar</div>}
        sidebar={<div>Sidebar</div>}
        list={<div>List</div>}
        detail={<div>Detail</div>}
      />,
    );

    const sidebar = screen.getByTestId("three-pane-sidebar");
    expect(sidebar).toHaveStyle({ width: "248px" });

    act(() => useUiStore.getState().setSidebarCollapsed(true));

    expect(sidebar).toHaveStyle({ width: "56px" });
  });

  it("uses the same collapsed width on the settings route", () => {
    render(
      <SettingsRouteLayout topBar={<div>Top bar</div>} sidebar={<div>Sidebar</div>}>
        <div>Settings content</div>
      </SettingsRouteLayout>,
    );

    const sidebar = screen.getByTestId("settings-route-sidebar");
    expect(sidebar).toHaveStyle({ width: "248px" });
    expect(screen.getByText("Settings content")).toBeInTheDocument();

    act(() => useUiStore.getState().setSidebarCollapsed(true));

    expect(sidebar).toHaveStyle({ width: "56px" });
  });
});
