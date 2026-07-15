import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useUiStore } from "../../store/uiStore";
import type { StartupStatus } from "../../types/api";
import { StartupRecoveryScreen } from "./StartupRecoveryScreen";

vi.mock("../../hooks/commands/useRestartApp", () => ({
  useRestartApp: () => ({
    error: undefined,
    loading: false,
    restartApp: vi.fn(),
  }),
}));

vi.mock("../settings/StorageSettings", () => ({
  StorageSettings: () => <div>Storage recovery settings</div>,
}));

const recoveryStatus: StartupStatus = {
  mode: "recovery",
  backendVersion: "0.1.0",
  issue: {
    code: "ERR_DB_MIGRATION",
    title: "Database migration failed",
    message: "Migration checksum mismatch",
    recoveryKind: "database_migration",
  },
};

beforeEach(() => {
  window.localStorage.clear();
  useUiStore.setState({ locale: "en", theme: "dark" });
});

describe("StartupRecoveryScreen", () => {
  it("keeps the frameless window draggable and controllable during recovery", () => {
    render(<StartupRecoveryScreen status={recoveryStatus} />);

    expect(screen.getByTestId("window-titlebar")).toHaveAttribute(
      "data-tauri-drag-region",
      "deep",
    );
    expect(screen.getByRole("heading", { name: "拾海 · Node Tide" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Window controls" })).toBeInTheDocument();
    expect(screen.getByText("Migration checksum mismatch")).toBeInTheDocument();
    expect(screen.getByText("Storage recovery settings")).toBeInTheDocument();
  });
});
