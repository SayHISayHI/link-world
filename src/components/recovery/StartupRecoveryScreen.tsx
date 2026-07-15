import { AlertTriangle, RefreshCw, ShieldCheck } from "lucide-react";
import { useRestartApp } from "../../hooks/commands/useRestartApp";
import type { StartupIssue, StartupStatus } from "../../types/api";
import { AppShell } from "../layout/AppShell";
import { WindowTitleBar } from "../layout/WindowTitleBar";
import { StorageSettings } from "../settings/StorageSettings";
import { Button } from "../ui/button";
import { useI18n, type Translator } from "../../i18n";

interface StartupRecoveryScreenProps {
  status: StartupStatus;
}

export function StartupRecoveryScreen({ status }: StartupRecoveryScreenProps) {
  const { t } = useI18n();
  const issue = status.issue;
  const { error, loading, restartApp } = useRestartApp();

  return (
    <AppShell>
      <div className="flex h-screen flex-col overflow-hidden bg-background">
        <WindowTitleBar />
        <div className="min-h-0 flex-1 overflow-y-auto">
          <section className="border-b border-border bg-surface">
            <div className="mx-auto max-w-5xl px-8 py-7">
              <div className="flex items-start justify-between gap-5">
                <div className="flex gap-4">
                  <div className="mt-1 flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-950/50 text-amber-800 dark:text-amber-200">
                    <AlertTriangle className="h-5 w-5" aria-hidden="true" />
                  </div>
                  <div>
                    <p className="text-xs font-medium uppercase tracking-wide text-amber-700 dark:text-amber-300">
                      {t("Restricted startup recovery")}
                    </p>
                    <h1 className="mt-1 text-2xl font-semibold">
                      {t("Node Tide could not open the normal library safely.")}
                    </h1>
                    <p className="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">
                      {issue?.message ??
                        t("Startup failed before the library database and background services were opened.")}
                    </p>
                  </div>
                </div>
                <Button
                  variant="secondary"
                  onClick={() => void restartApp()}
                  disabled={loading}
                >
                  <RefreshCw
                    className={loading ? "h-4 w-4 animate-spin" : "h-4 w-4"}
                    aria-hidden="true"
                  />
                  {loading ? t("Restarting...") : t("Restart and retry")}
                </Button>
              </div>

              {issue ? <RecoveryIssueCard issue={issue} /> : null}
              {error ? (
                <div className="mt-4 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm text-red-800 dark:text-red-200">
                  <p className="font-medium">{t(error.title)}</p>
                  <p className="mt-1">{t(error.message)}</p>
                </div>
              ) : null}
            </div>
          </section>

          <StorageSettings mode="startupRecovery" startupIssue={issue} />
        </div>
      </div>
    </AppShell>
  );
}

function RecoveryIssueCard({ issue }: { issue: StartupIssue }) {
  const { t } = useI18n();
  return (
    <div
      className="mt-5 grid gap-3 rounded-lg border border-border bg-background p-4 text-xs leading-5 text-muted-foreground md:grid-cols-3"
      role="alert"
    >
      <div>
        <p className="font-medium text-foreground">{t("Failure kind")}</p>
        <p className="mt-1">{recoveryKindLabel(issue.recoveryKind, t)}</p>
      </div>
      <div>
        <p className="font-medium text-foreground">{t("Error code")}</p>
        <p className="mt-1">{issue.code}</p>
      </div>
      <div>
        <p className="font-medium text-foreground">{t("Safe restore point")}</p>
        <p className="mt-1">{issue.verifiedBackupId ?? t("Not available from startup guard")}</p>
      </div>
      {issue.migration ? (
        <div className="flex gap-2 rounded-md border border-emerald-200 dark:border-emerald-900 bg-emerald-50 dark:bg-emerald-950/30 p-3 text-emerald-900 dark:text-emerald-200 md:col-span-3">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
          <p>
            {t("Migration guard phase {phase}; target schema version {version}. The guard exposes only restore-point metadata.", { phase: issue.migration.phase, version: issue.migration.targetVersion })}
          </p>
        </div>
      ) : null}
    </div>
  );
}

function recoveryKindLabel(kind: StartupIssue["recoveryKind"], t: Translator) {
  switch (kind) {
    case "database_migration":
      return t("Database migration");
    case "restore":
      return t("Restore transaction");
    case "database":
      return t("Database open");
    case "storage":
      return t("Local storage");
    default:
      return t("Unknown startup failure");
  }
}
