import { useEffect, useState } from "react";
import { Archive, RefreshCw, RotateCcw, ShieldCheck } from "lucide-react";
import { useBackups } from "../../hooks/commands/useBackups";
import type { BackupSummary } from "../../types/api";
import { Button } from "../ui/button";

export function StorageSettings() {
  const {
    backups,
    creating,
    error,
    loading,
    restoreStatus,
    restoringId,
    verificationById,
    verifyingId,
    createBackup,
    loadBackups,
    loadRestoreStatus,
    restoreBackup,
    verifyBackup,
  } = useBackups();
  const [confirmingId, setConfirmingId] = useState<string>();

  useEffect(() => {
    void Promise.all([loadBackups(), loadRestoreStatus()]);
  }, [loadBackups, loadRestoreStatus]);

  return (
    <div className="mx-auto max-w-5xl p-8">
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="text-xl font-semibold">Storage and backups</h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
            Create a local restore point containing a consistent SQLite snapshot and the object
            store. Every payload file is recorded in a versioned SHA-256 manifest.
          </p>
        </div>
        <Button onClick={() => void createBackup()} disabled={creating || loading}>
          <Archive className="h-4 w-4" aria-hidden="true" />
          {creating ? "Creating..." : "Create backup"}
        </Button>
      </div>

      <div className="mt-6 rounded-lg border border-amber-200 bg-amber-50 p-4 text-xs leading-5 text-amber-900">
        Backups contain saved user content, including locally classified sensitive content. API
        key values remain in Windows Credential Manager and are never copied. Protect the Windows
        account and backup directory accordingly.
      </div>

      {error ? (
        <div className="mt-5 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800">
          <p className="font-medium">{error.title}</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {restoreStatus ? (
        <div
          className={
            "mt-5 rounded-md border p-3 text-sm " +
            (restoreStatus.status === "succeeded"
              ? "border-emerald-200 bg-emerald-50 text-emerald-800"
              : "border-amber-200 bg-amber-50 text-amber-900")
          }
        >
          <p className="font-medium">
            {restoreStatus.status === "succeeded"
              ? "Restore completed"
              : "Restore did not replace the current library"}
          </p>
          <p className="mt-1 text-xs">
            Target {restoreStatus.backupId} / safety backup {restoreStatus.safetyBackupId} /{" "}
            {formatDate(restoreStatus.completedAt)}
          </p>
          {restoreStatus.message ? (
            <p className="mt-1 text-xs">{restoreStatus.message}</p>
          ) : null}
        </div>
      ) : null}

      <section className="mt-7">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold">Local restore points</h3>
          <Button
            variant="ghost"
            className="h-8 px-2 text-xs"
            onClick={() => void loadBackups()}
            disabled={loading || creating}
          >
            <RefreshCw
              className={loading ? "h-4 w-4 animate-spin" : "h-4 w-4"}
              aria-hidden="true"
            />
            Refresh
          </Button>
        </div>

        <div className="mt-3 space-y-3">
          {loading && backups.length === 0 ? (
            <p className="text-sm text-muted-foreground">Loading backups...</p>
          ) : null}
          {!loading && backups.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground">
              No local restore points yet.
            </div>
          ) : null}
          {backups.map((backup) => (
            <BackupRow
              key={backup.backupId}
              backup={backup}
              confirming={confirmingId === backup.backupId}
              restoring={restoringId === backup.backupId}
              restoreDisabled={restoringId !== undefined}
              verification={verificationById[backup.backupId]}
              verifying={verifyingId === backup.backupId}
              onCancelRestore={() => setConfirmingId(undefined)}
              onRequestRestore={() => setConfirmingId(backup.backupId)}
              onRestore={() => void restoreBackup(backup.backupId)}
              onVerify={() => void verifyBackup(backup.backupId)}
            />
          ))}
        </div>
      </section>

      <div className="mt-7 flex gap-3 rounded-lg border border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
        <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
        <p>
          Restore preparation re-verifies every payload, migrates a private candidate, and creates
          a safety backup. Live data is switched only during restart; interrupted or invalid
          restores are rolled back before the application opens.
        </p>
      </div>
    </div>
  );
}

function BackupRow({
  backup,
  confirming,
  onCancelRestore,
  onRequestRestore,
  onRestore,
  onVerify,
  restoring,
  restoreDisabled,
  verification,
  verifying,
}: {
  backup: BackupSummary;
  confirming: boolean;
  onCancelRestore: () => void;
  onRequestRestore: () => void;
  onRestore: () => void;
  onVerify: () => void;
  restoring: boolean;
  restoreDisabled: boolean;
  verification?: {
    valid: boolean;
    checkedFileCount: number;
    issues: string[];
  };
  verifying: boolean;
}) {
  const invalid = backup.status === "invalid";

  return (
    <div className="rounded-lg border border-border bg-surface p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <p className="truncate text-sm font-medium">{backup.backupId}</p>
            <span
              className={
                "rounded-full px-2 py-0.5 text-[11px] " +
                (invalid
                  ? "bg-red-100 text-red-800"
                  : "bg-emerald-100 text-emerald-800")
              }
            >
              {invalid ? "Invalid manifest" : "Ready"}
            </span>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            {formatDate(backup.createdAt)} / {formatBytes(backup.totalSizeBytes)} /{" "}
            {backup.objectFileCount} object files
          </p>
          {backup.appVersion ? (
            <p className="mt-1 text-[11px] text-muted-foreground">
              Link World {backup.appVersion}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            variant="secondary"
            className="h-8 text-xs"
            onClick={onVerify}
            disabled={verifying || invalid || restoring}
          >
            {verifying ? "Verifying..." : "Verify"}
          </Button>
          <Button
            variant="secondary"
            className="h-8 text-xs"
            onClick={onRequestRestore}
            disabled={invalid || restoring || restoreDisabled}
          >
            <RotateCcw className="h-3.5 w-3.5" aria-hidden="true" />
            {restoring ? "Preparing..." : "Restore"}
          </Button>
        </div>
      </div>
      {verification ? (
        verification.valid ? (
          <p className="mt-3 rounded-md border border-emerald-200 bg-emerald-50 p-2 text-xs text-emerald-800">
            Verified {verification.checkedFileCount} payload files and SQLite integrity.
          </p>
        ) : (
          <div className="mt-3 rounded-md border border-red-200 bg-red-50 p-2 text-xs text-red-800">
            <p className="font-medium">Verification failed</p>
            <ul className="mt-1 list-disc pl-4">
              {verification.issues.map((issue) => (
                <li key={issue}>{issue}</li>
              ))}
            </ul>
          </div>
        )
      ) : null}
      {confirming ? (
        <div
          className="mt-3 rounded-md border border-amber-300 bg-amber-50 p-3 text-xs text-amber-950"
          role="group"
          aria-label="Confirm restore"
        >
          <p className="font-semibold">Restore this point and restart Link World?</p>
          <p className="mt-1 leading-5">
            The current database and object store will be replaced. Before shutdown, Link World
            re-verifies this backup, migrates a private candidate, and creates a separate safety
            backup for automatic rollback.
          </p>
          <div className="mt-3 flex justify-end gap-2">
            <Button
              variant="ghost"
              className="h-8 text-xs"
              onClick={onCancelRestore}
              disabled={restoring}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              className="h-8 text-xs"
              onClick={onRestore}
              disabled={restoring}
            >
              {restoring ? "Preparing and restarting..." : "Restore and restart"}
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function formatDate(value?: string) {
  if (!value) {
    return "Unknown creation time";
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function formatBytes(value: number) {
  if (value < 1024) {
    return value + " B";
  }
  if (value < 1024 * 1024) {
    return (value / 1024).toFixed(1) + " KB";
  }
  return (value / (1024 * 1024)).toFixed(1) + " MB";
}
