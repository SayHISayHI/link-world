import { AlertTriangle, RefreshCw, ShieldCheck } from "lucide-react";
import { useRestartApp } from "../../hooks/commands/useRestartApp";
import type { StartupIssue, StartupStatus } from "../../types/api";
import { AppShell } from "../layout/AppShell";
import { StorageSettings } from "../settings/StorageSettings";
import { Button } from "../ui/button";

interface StartupRecoveryScreenProps {
  status: StartupStatus;
}

export function StartupRecoveryScreen({ status }: StartupRecoveryScreenProps) {
  const issue = status.issue;
  const { error, loading, restartApp } = useRestartApp();

  return (
    <AppShell>
      <div className="min-h-screen bg-background">
        <section className="border-b border-border bg-surface">
          <div className="mx-auto max-w-5xl px-8 py-7">
            <div className="flex items-start justify-between gap-5">
              <div className="flex gap-4">
                <div className="mt-1 flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-amber-100 text-amber-800">
                  <AlertTriangle className="h-5 w-5" aria-hidden="true" />
                </div>
                <div>
                  <p className="text-xs font-medium uppercase tracking-wide text-amber-700">
                    Restricted startup recovery
                  </p>
                  <h1 className="mt-1 text-2xl font-semibold">
                    Link World could not open the normal library safely.
                  </h1>
                  <p className="mt-3 max-w-3xl text-sm leading-6 text-muted-foreground">
                    {issue?.message ??
                      "Startup failed before the library database and background services were opened."}
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
                {loading ? "Restarting..." : "Restart and retry"}
              </Button>
            </div>

            {issue ? <RecoveryIssueCard issue={issue} /> : null}
            {error ? (
              <div className="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800">
                <p className="font-medium">{error.title}</p>
                <p className="mt-1">{error.message}</p>
              </div>
            ) : null}
          </div>
        </section>

        <StorageSettings mode="startupRecovery" startupIssue={issue} />
      </div>
    </AppShell>
  );
}

function RecoveryIssueCard({ issue }: { issue: StartupIssue }) {
  return (
    <div
      className="mt-5 grid gap-3 rounded-lg border border-border bg-background p-4 text-xs leading-5 text-muted-foreground md:grid-cols-3"
      role="alert"
    >
      <div>
        <p className="font-medium text-foreground">Failure kind</p>
        <p className="mt-1">{recoveryKindLabel(issue.recoveryKind)}</p>
      </div>
      <div>
        <p className="font-medium text-foreground">Error code</p>
        <p className="mt-1">{issue.code}</p>
      </div>
      <div>
        <p className="font-medium text-foreground">Safe restore point</p>
        <p className="mt-1">{issue.verifiedBackupId ?? "Not available from startup guard"}</p>
      </div>
      {issue.migration ? (
        <div className="flex gap-2 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-emerald-900 md:col-span-3">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
          <p>
            Migration guard phase {issue.migration.phase}; target schema version{" "}
            {issue.migration.targetVersion}. The guard exposes only restore-point metadata.
          </p>
        </div>
      ) : null}
    </div>
  );
}

function recoveryKindLabel(kind: StartupIssue["recoveryKind"]) {
  switch (kind) {
    case "database_migration":
      return "Database migration";
    case "restore":
      return "Restore transaction";
    case "database":
      return "Database open";
    case "storage":
      return "Local storage";
    default:
      return "Unknown startup failure";
  }
}
