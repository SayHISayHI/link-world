import { useEffect, useState } from "react";
import { Archive, Download, RefreshCw, RotateCcw, ShieldCheck } from "lucide-react";
import { useBackups } from "../../hooks/commands/useBackups";
import { usePortableExport } from "../../hooks/commands/usePortableExport";
import type { BackupSummary, StartupIssue } from "../../types/api";
import { Button } from "../ui/button";
import { localeTag, useI18n, type Translator } from "../../i18n";

interface StorageSettingsProps {
  mode?: "settings" | "startupRecovery";
  startupIssue?: StartupIssue;
}

export function StorageSettings({ mode = "settings", startupIssue }: StorageSettingsProps) {
  const { locale, t } = useI18n();
  const isStartupRecovery = mode === "startupRecovery";
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
  const {
    error: exportError,
    exporting,
    summary: exportSummary,
    exportLibrary,
  } = usePortableExport();
  const [confirmingId, setConfirmingId] = useState<string>();
  const visibleError = error ?? exportError;

  useEffect(() => {
    void Promise.all([loadBackups(), loadRestoreStatus()]);
  }, [loadBackups, loadRestoreStatus]);

  return (
    <div className="mx-auto max-w-5xl p-8">
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="text-xl font-semibold">
            {isStartupRecovery ? t("Startup recovery") : t("Storage and backups")}
          </h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
            {isStartupRecovery
              ? t("Node Tide is running in restricted recovery mode. Choose a verified local restore point to recover the library; ordinary library features stay disabled until restart succeeds.")
              : t("Create a local restore point containing a consistent SQLite snapshot and the object store. Every payload file is recorded in a versioned SHA-256 manifest.")}
          </p>
        </div>
        {isStartupRecovery ? null : (
          <Button onClick={() => void createBackup()} disabled={creating || loading}>
            <Archive className="h-4 w-4" aria-hidden="true" />
            {creating ? t("Creating...") : t("Create backup")}
          </Button>
        )}
      </div>

      {isStartupRecovery && startupIssue?.verifiedBackupId ? (
        <div className="mt-5 rounded-lg border border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/30 p-4 text-xs leading-5 text-emerald-900 dark:text-emerald-200">
          {t("Verified pre-migration restore point available: {id}", { id: startupIssue.verifiedBackupId })}
        </div>
      ) : null}

      <div className="mt-6 rounded-lg border border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/30 p-4 text-xs leading-5 text-amber-900 dark:text-amber-200">
        {t("Backups contain saved user content, including locally classified sensitive content. API key values remain in Windows Credential Manager and are never copied. Protect the Windows account and backup directory accordingly.")}
      </div>

      {visibleError ? (
        <div className="mt-5 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm text-red-800 dark:text-red-200">
          <p className="font-medium">{t(visibleError.title)}</p>
          <p className="mt-1">{t(visibleError.message)}</p>
        </div>
      ) : null}

      {restoreStatus ? (
        <div
          className={
            "mt-5 rounded-md border p-3 text-sm " +
            (restoreStatus.status === "succeeded"
              ? "border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/30 text-emerald-800 dark:text-emerald-200"
              : "border-amber-200 dark:border-amber-900 bg-amber-50 dark:bg-amber-950/30 text-amber-900 dark:text-amber-200")
          }
        >
          <p className="font-medium">
            {restoreStatus.status === "succeeded"
              ? t("Restore completed")
              : t("Restore did not replace the current library")}
          </p>
          <p className="mt-1 text-xs">
            {t("Target {target} / safety backup {safety} / {date}", { target: restoreStatus.backupId, safety: restoreStatus.safetyBackupId, date: formatDate(restoreStatus.completedAt, locale, t) })}
          </p>
          {restoreStatus.message ? (
            <p className="mt-1 text-xs">{t(restoreStatus.message)}</p>
          ) : null}
        </div>
      ) : null}

      {isStartupRecovery ? null : (
        <section className="mt-7 rounded-lg border border-border bg-surface p-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h3 className="text-sm font-semibold">{t("Portable export")}</h3>
              <p className="mt-2 max-w-2xl text-xs leading-5 text-muted-foreground">
                {t("Export all non-secret objects to Markdown and JSON under the app data exports folder. Credential references, internal jobs, and local object storage paths are excluded.")}
              </p>
            </div>
            <Button
              variant="secondary"
              onClick={() => void exportLibrary()}
              disabled={exporting || loading || creating}
            >
              <Download className="h-4 w-4" aria-hidden="true" />
              {exporting ? t("Exporting...") : t("Export library")}
            </Button>
          </div>
          {exportSummary ? (
            <div className="mt-4 rounded-md border border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/30 p-3 text-xs leading-5 text-emerald-900 dark:text-emerald-200">
              {t("Exported {count} objects to {path}. Skipped {skipped} secret objects by default.", { count: exportSummary.objectCount, path: exportSummary.exportRoot, skipped: exportSummary.skippedSecretCount })}
            </div>
          ) : null}
        </section>
      )}

      <section className="mt-7">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-sm font-semibold">{t("Local restore points")}</h3>
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
            {t("Refresh")}
          </Button>
        </div>

        <div className="mt-3 space-y-3">
          {loading && backups.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("Loading backups...")}</p>
          ) : null}
          {!loading && backups.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground">
              {t("No local restore points yet.")}
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
          {isStartupRecovery
            ? t("Recovery mode only exposes backup listing, verification, restore preparation, and restart. It does not open the normal library database or start background services.")
            : t("Restore preparation re-verifies every payload, migrates a private candidate, and creates a safety backup. Live data is switched only during restart; interrupted or invalid restores are rolled back before the application opens.")}
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
  const { locale, t } = useI18n();
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
                  ? "bg-red-100 dark:bg-red-950/50 text-red-800 dark:text-red-200"
                  : "bg-emerald-100 dark:bg-emerald-950/50 text-emerald-800 dark:text-emerald-200")
              }
            >
              {invalid ? t("Invalid manifest") : t("Ready")}
            </span>
          </div>
          <p className="mt-2 text-xs text-muted-foreground">
            {t("{date} / {size} / {count} object files", { date: formatDate(backup.createdAt, locale, t), size: formatBytes(backup.totalSizeBytes), count: backup.objectFileCount })}
          </p>
          {backup.appVersion ? (
            <p className="mt-1 text-[11px] text-muted-foreground">
              Node Tide {backup.appVersion}
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
            {verifying ? t("Verifying...") : t("Verify")}
          </Button>
          <Button
            variant="secondary"
            className="h-8 text-xs"
            onClick={onRequestRestore}
            disabled={invalid || restoring || restoreDisabled}
          >
            <RotateCcw className="h-3.5 w-3.5" aria-hidden="true" />
            {restoring ? t("Preparing...") : t("Restore")}
          </Button>
        </div>
      </div>
      {verification ? (
        verification.valid ? (
          <p className="mt-3 rounded-md border border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/30 p-2 text-xs text-emerald-800 dark:text-emerald-200">
            {t("Verified {count} payload files and SQLite integrity.", { count: verification.checkedFileCount })}
          </p>
        ) : (
          <div className="mt-3 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-2 text-xs text-red-800 dark:text-red-200">
            <p className="font-medium">{t("Verification failed")}</p>
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
          className="mt-3 rounded-md border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-950/30 p-3 text-xs text-amber-950 dark:text-amber-100"
          role="group"
          aria-label={t("Confirm restore")}
        >
          <p className="font-semibold">{t("Restore this point and restart Node Tide?")}</p>
          <p className="mt-1 leading-5">
            {t("The current database and object store will be replaced. Before shutdown, Node Tide re-verifies this backup, migrates a private candidate, and creates a separate safety backup for automatic rollback.")}
          </p>
          <div className="mt-3 flex justify-end gap-2">
            <Button
              variant="ghost"
              className="h-8 text-xs"
              onClick={onCancelRestore}
              disabled={restoring}
            >
              {t("Cancel")}
            </Button>
            <Button
              variant="primary"
              className="h-8 text-xs"
              onClick={onRestore}
              disabled={restoring}
            >
              {restoring ? t("Preparing and restarting...") : t("Restore and restart")}
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function formatDate(value: string | undefined, locale: "en" | "zh-CN", t: Translator) {
  if (!value) {
    return t("Unknown creation time");
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(localeTag(locale));
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
