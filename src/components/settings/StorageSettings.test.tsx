import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StorageSettings } from "./StorageSettings";

const useBackupsMock = vi.hoisted(() => vi.fn());

const usePortableExportMock = vi.hoisted(() => vi.fn());
vi.mock("../../hooks/commands/useBackups", () => ({
  useBackups: useBackupsMock,
}));

vi.mock("../../hooks/commands/usePortableExport", () => ({
  usePortableExport: usePortableExportMock,
}));

const createBackup = vi.fn();
const exportLibrary = vi.fn();
const loadBackups = vi.fn();
const loadRestoreStatus = vi.fn();
const restoreBackup = vi.fn();
const verifyBackup = vi.fn();

function hookState(overrides: Record<string, unknown> = {}) {
  return {
    backups: [],
    creating: false,
    loading: false,
    verificationById: {},
    createBackup,
    loadBackups,
    loadRestoreStatus,
    restoreBackup,
    verifyBackup,
    ...overrides,
  };
}

beforeEach(() => {
  createBackup.mockReset();
  exportLibrary.mockReset();
  loadBackups.mockReset();
  loadRestoreStatus.mockReset();
  restoreBackup.mockReset();
  verifyBackup.mockReset();
  usePortableExportMock.mockReturnValue({
    exporting: false,
    exportLibrary,
  });
  useBackupsMock.mockReturnValue(hookState());
});

describe("StorageSettings", () => {
  it("loads storage state and creates a local restore point", () => {
    render(<StorageSettings />);

    expect(loadBackups).toHaveBeenCalledOnce();
    expect(loadRestoreStatus).toHaveBeenCalledOnce();
    expect(screen.getByText(/Backups contain saved user content/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Create backup" }));
    expect(createBackup).toHaveBeenCalledOnce();
  });

  it("exports a portable non-secret Markdown and JSON copy", () => {
    usePortableExportMock.mockReturnValue({
      exporting: false,
      exportLibrary,
      summary: {
        exportId: "export-1",
        exportRoot: "C:/Users/example/AppData/Roaming/link-world/exports/export-1",
        format: "markdown_json_directory",
        objectCount: 2,
        skippedSecretCount: 1,
        markdownFileCount: 2,
        jsonFileCount: 4,
        createdAt: "2026-06-25T00:00:00Z",
      },
    });

    render(<StorageSettings />);

    fireEvent.click(screen.getByRole("button", { name: "Export library" }));
    expect(exportLibrary).toHaveBeenCalledOnce();
    expect(screen.getByText(/Exported 2 objects/)).toBeInTheDocument();
    expect(screen.getByText(/Skipped 1 secret objects/)).toBeInTheDocument();
  });

  it("shows verification metadata without exposing payload content", () => {
    useBackupsMock.mockReturnValue(
      hookState({
        backups: [
          {
            backupId: "backup-1",
            appVersion: "0.1.0",
            createdAt: "2026-06-23T00:00:00Z",
            objectFileCount: 3,
            totalSizeBytes: 4096,
            status: "ready",
          },
        ],
        verificationById: {
          "backup-1": {
            backupId: "backup-1",
            valid: true,
            checkedFileCount: 4,
            issues: [],
          },
        },
      }),
    );

    render(<StorageSettings />);

    expect(screen.getByText("Verified 4 payload files and SQLite integrity.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Verify" }));
    expect(verifyBackup).toHaveBeenCalledWith("backup-1");
    expect(screen.queryByText(/snapshot-1/)).not.toBeInTheDocument();
  });

  it("requires explicit confirmation before preparing a restore", () => {
    useBackupsMock.mockReturnValue(
      hookState({
        backups: [
          {
            backupId: "backup-1",
            appVersion: "0.1.0",
            createdAt: "2026-06-23T00:00:00Z",
            objectFileCount: 1,
            totalSizeBytes: 2048,
            status: "ready",
          },
        ],
      }),
    );

    render(<StorageSettings />);

    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(restoreBackup).not.toHaveBeenCalled();
    expect(screen.getByRole("group", { name: "Confirm restore" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Restore and restart" }));
    expect(restoreBackup).toHaveBeenCalledWith("backup-1");
  });

  it("surfaces an automatic rollback result and safety backup id", () => {
    useBackupsMock.mockReturnValue(
      hookState({
        restoreStatus: {
          backupId: "backup-target",
          safetyBackupId: "backup-safety",
          status: "rolled_back",
          completedAt: "2026-06-24T00:00:00Z",
          message: "database integrity check failed",
        },
      }),
    );

    render(<StorageSettings />);

    expect(
      screen.getByText("Restore did not replace the current library"),
    ).toBeInTheDocument();
    expect(screen.getByText(/backup-safety/)).toBeInTheDocument();
    expect(screen.getByText("database integrity check failed")).toBeInTheDocument();
  });

  it("limits startup recovery mode to restore actions", () => {
    useBackupsMock.mockReturnValue(
      hookState({
        backups: [
          {
            backupId: "backup-guard",
            appVersion: "0.1.0",
            createdAt: "2026-06-23T00:00:00Z",
            objectFileCount: 2,
            totalSizeBytes: 8192,
            status: "ready",
          },
        ],
      }),
    );

    render(
      <StorageSettings
        mode="startupRecovery"
        startupIssue={{
          code: "ERR_DB_MIGRATION",
          title: "Database migration needs recovery",
          message: "automatic retry is blocked",
          recoveryKind: "database_migration",
          verifiedBackupId: "backup-guard",
        }}
      />,
    );

    expect(screen.getByText("Startup recovery")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Create backup" })).not.toBeInTheDocument();
    expect(screen.getAllByText(/backup-guard/).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Export library" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore" })).toBeInTheDocument();
  });
});
