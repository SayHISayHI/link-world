import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { useLocalMetricsSnapshot } from "../../hooks/commands/useLocalMetricsSnapshot";
import { useRetryBackgroundJob } from "../../hooks/commands/useRetryBackgroundJob";
import { useSupportBundleExport } from "../../hooks/commands/useSupportBundleExport";
import type {
  FailedJobSummary,
  LocalMetricsSnapshot,
  SupportBundleSummary,
} from "../../types/api";
import { Button } from "../ui/button";

interface DiagnosticsSettingsProps {
  onOpenObject?: (objectId: string) => void;
}

export function DiagnosticsSettings({ onOpenObject }: DiagnosticsSettingsProps) {
  const { data, error, loading, loadSnapshot } = useLocalMetricsSnapshot();
  const {
    error: retryError,
    loading: retryLoading,
    retryBackgroundJob,
  } = useRetryBackgroundJob();
  const {
    error: supportBundleError,
    exporting: supportBundleExporting,
    exportSupportBundle,
    summary: supportBundleSummary,
  } = useSupportBundleExport();
  const [supportBundleConfirmed, setSupportBundleConfirmed] = useState(false);

  useEffect(() => {
    void loadSnapshot();
  }, [loadSnapshot]);

  const retryJob = async (jobId: string) => {
    const retried = await retryBackgroundJob({ jobId });
    if (retried) {
      await loadSnapshot();
    }
  };

  const exportBundle = async () => {
    if (!supportBundleConfirmed) {
      return;
    }

    const summary = await exportSupportBundle({ confirmed: true });
    if (summary) {
      setSupportBundleConfirmed(false);
    }
  };

  return (
    <div className="mx-auto max-w-6xl p-8">
      <div className="flex items-start justify-between gap-5">
        <div>
          <h2 className="text-xl font-semibold">Diagnostics</h2>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
            Local runtime health, storage status, failed job summaries, and redaction boundaries.
            This page displays local diagnostics only; support bundle export requires explicit confirmation below.
          </p>
        </div>
        <Button variant="secondary" onClick={() => void loadSnapshot()} disabled={loading}>
          <RefreshCw className={loading ? "h-4 w-4 animate-spin" : "h-4 w-4"} aria-hidden="true" />
          {loading ? "Refreshing..." : "Refresh"}
        </Button>
      </div>

      {error ? (
        <div className="mt-6 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-800">
          <p className="font-medium">{error.title}</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {retryError ? (
        <div className="mt-6 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-800">
          <p className="font-medium">{retryError.title}</p>
          <p className="mt-1">{retryError.message}</p>
        </div>
      ) : null}

      {supportBundleError ? (
        <div className="mt-6 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-800">
          <p className="font-medium">{supportBundleError.title}</p>
          <p className="mt-1">{supportBundleError.message}</p>
        </div>
      ) : null}

      {!data && loading ? <p className="mt-6 text-sm text-muted-foreground">Loading diagnostics...</p> : null}
      {data ? (
        <div className="mt-7 space-y-6">
          <HealthGrid snapshot={data} />
          <FailedJobs
            jobs={data.jobs.recentFailures}
            onOpenObject={onOpenObject}
            onRetryJob={(jobId) => void retryJob(jobId)}
            retryLoading={retryLoading}
          />
          <PrivacyBoundary
            snapshot={data}
            confirmed={supportBundleConfirmed}
            exporting={supportBundleExporting}
            summary={supportBundleSummary}
            onConfirmedChange={setSupportBundleConfirmed}
            onExport={() => void exportBundle()}
          />
        </div>
      ) : null}
    </div>
  );
}

function HealthGrid({ snapshot }: { snapshot: LocalMetricsSnapshot }) {
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <section className="rounded-xl border border-border bg-surface p-5">
        <h3 className="text-sm font-semibold">Runtime</h3>
        <dl className="mt-4 space-y-3 text-sm">
          <Metric label="App version" value={snapshot.appVersion} />
          <Metric label="Data directory" value={snapshot.dataDir} mono />
          <Metric label="Database" value={snapshot.databasePath} mono />
          <Metric label="Object store" value={snapshot.objectStorePath} mono />
        </dl>
      </section>

      <section className="rounded-xl border border-border bg-surface p-5">
        <h3 className="text-sm font-semibold">Database health</h3>
        <dl className="mt-4 space-y-3 text-sm">
          <Metric label="Status" value={snapshot.databaseHealth.healthy ? "Healthy" : "Needs attention"} />
          <Metric label="SQLite quick_check" value={snapshot.databaseHealth.quickCheck} />
          <Metric label="Foreign key violations" value={snapshot.databaseHealth.foreignKeyViolations.toString()} />
          <Metric label="Migration version" value={snapshot.databaseHealth.appliedMigrationVersion?.toString() ?? "Unknown"} />
          <Metric label="Size" value={formatBytes(snapshot.databaseHealth.sizeBytes)} />
        </dl>
      </section>

      <section className="rounded-xl border border-border bg-surface p-5">
        <h3 className="text-sm font-semibold">Object store</h3>
        <dl className="mt-4 space-y-3 text-sm">
          <Metric label="Status" value={snapshot.objectStoreHealth.healthy ? "Healthy" : "Needs attention"} />
          <Metric label="Files" value={snapshot.objectStoreHealth.fileCount.toString()} />
          <Metric label="Size" value={formatBytes(snapshot.objectStoreHealth.sizeBytes)} />
          {snapshot.objectStoreHealth.issue ? <Metric label="Issue" value={snapshot.objectStoreHealth.issue} /> : null}
        </dl>
      </section>

      <section className="rounded-xl border border-border bg-surface p-5">
        <h3 className="text-sm font-semibold">Jobs and models</h3>
        <dl className="mt-4 space-y-3 text-sm">
          <Metric label="Queued / running" value={`${snapshot.jobs.queued} / ${snapshot.jobs.running}`} />
          <Metric label="Failed / blocked" value={`${snapshot.jobs.failed} / ${snapshot.jobs.blocked}`} />
          <Metric label="Cancelled" value={snapshot.jobs.cancelled.toString()} />
          <Metric label="Model configs" value={`${snapshot.models.enabledCount}/${snapshot.models.configuredCount} enabled`} />
          <Metric label="Default chat model" value={modelStatusLabel(snapshot.models.status)} />
        </dl>
        {snapshot.models.status === "not_configured_normal_degradation" ? (
          <p className="mt-3 rounded-md border border-border bg-background p-3 text-xs leading-5 text-muted-foreground">
            No model provider is configured. Save, parse, search, backup, restore, and diagnostics remain healthy; AI features are degraded by design.
          </p>
        ) : null}
      </section>
    </div>
  );
}

function FailedJobs({
  jobs,
  onOpenObject,
  onRetryJob,
  retryLoading,
}: {
  jobs: FailedJobSummary[];
  onOpenObject?: (objectId: string) => void;
  onRetryJob: (jobId: string) => void;
  retryLoading: boolean;
}) {
  return (
    <section className="rounded-xl border border-border bg-surface p-5">
      <h3 className="text-sm font-semibold">Recent failed jobs</h3>
      {jobs.length === 0 ? (
        <div className="mt-4 rounded-lg border border-dashed border-border p-5 text-sm text-muted-foreground">
          No failed or blocked jobs in the latest diagnostics snapshot.
        </div>
      ) : (
        <div className="mt-4 space-y-3">
          {jobs.map((job) => (
            <div key={job.jobId} className="rounded-lg border border-border bg-background p-4 text-sm">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p className="font-medium">{job.jobType}</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {job.status} / {job.updatedAt}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  {job.objectId && onOpenObject ? (
                    <Button variant="secondary" onClick={() => onOpenObject(job.objectId!)}>
                      Open object
                    </Button>
                  ) : null}
                  {job.jobType === "capture.fetch_url" ? (
                    <Button variant="secondary" onClick={() => onRetryJob(job.jobId)} disabled={retryLoading}>
                      {retryLoading ? "Retrying..." : "Retry"}
                    </Button>
                  ) : null}
                </div>
              </div>
              {job.objectId ? <p className="mt-3 text-xs text-muted-foreground">Object: {job.objectId}</p> : null}
              {job.lastError ? (
                <p className="mt-2 rounded-md border border-border bg-surface p-3 text-xs leading-5 text-muted-foreground">
                  {job.lastError}
                </p>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function PrivacyBoundary({
  snapshot,
  confirmed,
  exporting,
  summary,
  onConfirmedChange,
  onExport,
}: {
  snapshot: LocalMetricsSnapshot;
  confirmed: boolean;
  exporting: boolean;
  summary?: SupportBundleSummary;
  onConfirmedChange: (confirmed: boolean) => void;
  onExport: () => void;
}) {
  const available = snapshot.privacy.supportBundleAvailable;

  return (
    <section className="rounded-xl border border-border bg-surface p-5">
      <h3 className="text-sm font-semibold">Support bundle</h3>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">
        Export a local JSON file with operational metadata only. Node Tide never uploads the file
        automatically, and the export does not read object bodies.
      </p>
      <ul className="mt-4 list-disc space-y-2 pl-5 text-sm text-muted-foreground">
        {snapshot.privacy.redaction.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>

      {available ? (
        <fieldset className="mt-5 rounded-lg border border-border bg-background p-4">
          <legend className="px-1 text-sm font-medium">Confirm support bundle export</legend>
          <label className="flex items-start gap-3 text-sm leading-6">
            <input
              type="checkbox"
              className="mt-1 h-4 w-4"
              checked={confirmed}
              onChange={(event) => onConfirmedChange(event.target.checked)}
            />
            <span>
              I understand this creates a local diagnostic file containing app/runtime metadata,
              stable failed-job codes, plugin fingerprints, and recent audit actions.
            </span>
          </label>
          <Button
            className="mt-4"
            variant="secondary"
            onClick={onExport}
            disabled={!confirmed || exporting}
          >
            {exporting ? "Exporting..." : "Export support bundle"}
          </Button>
        </fieldset>
      ) : (
        <p className="mt-5 text-sm text-muted-foreground">
          Support bundle export is unavailable in this build.
        </p>
      )}

      {summary ? (
        <div className="mt-5 rounded-lg border border-border bg-background p-4 text-sm">
          <p className="font-medium">Support bundle exported</p>
          <dl className="mt-3 space-y-2">
            <Metric label="File" value={summary.filePath} mono />
            <Metric label="Size" value={formatBytes(summary.sizeBytes)} />
            <Metric label="SHA-256" value={summary.sha256} mono />
          </dl>
          <p className="mt-3 text-xs leading-5 text-muted-foreground">
            Review the JSON before sharing it. The file remains on this device until you move or
            delete it.
          </p>
        </div>
      ) : null}
    </section>
  );
}

function Metric({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid gap-1 sm:grid-cols-[160px_minmax(0,1fr)]">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={mono ? "break-all font-mono text-xs" : "break-words"}>{value}</dd>
    </div>
  );
}

function formatBytes(value?: number) {
  if (value === undefined) {
    return "Unknown";
  }

  if (value < 1024) {
    return `${value} B`;
  }

  const units = ["KB", "MB", "GB"];
  let scaled = value / 1024;
  let unitIndex = 0;
  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  return `${scaled.toFixed(1)} ${units[unitIndex]}`;
}

function modelStatusLabel(status: string) {
  if (status === "configured") {
    return "Configured";
  }

  if (status === "not_configured_normal_degradation") {
    return "Not configured - normal degradation";
  }

  if (status === "missing_default_chat_config") {
    return "Missing default chat config";
  }

  return status;
}
