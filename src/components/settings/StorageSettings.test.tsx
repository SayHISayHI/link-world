import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { StorageSettings } from "./StorageSettings";

const useBackupsMock = vi.hoisted(() => vi.fn());

vi.mock("../../hooks/commands/useBackups", () => ({
  useBackups: useBackupsMock,
}));

const createBackup = vi.fn();
const loadBackups = vi.fn();
const verifyBackup = vi.fn();

beforeEach(() => {
  createBackup.mockReset();
  loadBackups.mockReset();
  verifyBackup.mockReset();
  useBackupsMock.mockReturnValue({
    backups: [],
    creating: false,
    loading: false,
    verificationById: {},
    createBackup,
    loadBackups,
    verifyBackup,
  });
});

describe("StorageSettings", () => {
  it("loads backups and creates a local restore point", () => {
    render(<StorageSettings />);

    expect(loadBackups).toHaveBeenCalledOnce();
    expect(screen.getByText(/Backups contain saved user content/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Create backup" }));
    expect(createBackup).toHaveBeenCalledOnce();
  });

  it("shows verification metadata without exposing payload content", () => {
    useBackupsMock.mockReturnValue({
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
      creating: false,
      loading: false,
      verificationById: {
        "backup-1": {
          backupId: "backup-1",
          valid: true,
          checkedFileCount: 4,
          issues: [],
        },
      },
      createBackup,
      loadBackups,
      verifyBackup,
    });

    render(<StorageSettings />);

    expect(screen.getByText("Verified 4 payload files and SQLite integrity.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Verify" }));
    expect(verifyBackup).toHaveBeenCalledWith("backup-1");
    expect(screen.queryByText(/snapshot-1/)).not.toBeInTheDocument();
  });
});
