import { Plus } from "lucide-react";
import type { AppUiError } from "../../lib/errors";
import { formatCaptureFailureReason } from "../../lib/captureFailures";
import { Button } from "../ui/button";
import { useI18n } from "../../i18n";

interface CaptureBarProps {
  value: string;
  loading: boolean;
  error?: AppUiError;
  job?: {
    status: string;
    lifecycleStatus?: string;
    failureReason?: string;
  };
  onChange: (value: string) => void;
  onSubmit: () => void;
}

export function CaptureBar({ value, loading, error, job, onChange, onSubmit }: CaptureBarProps) {
  const { t } = useI18n();
  const failure = job?.failureReason
    ? formatCaptureFailureReason(job.failureReason)
    : undefined;
  const isDeduplicated = job?.status === "deduplicated";
  const jobTone =
    job?.status === "failed"
      ? "bg-red-50 dark:bg-red-950/30 text-red-800 dark:text-red-200"
      : job?.status === "succeeded" || isDeduplicated
        ? "bg-emerald-50 dark:bg-emerald-950/30 text-emerald-800 dark:text-emerald-200"
        : "bg-muted text-muted-foreground";

  return (
    <form
      className="mb-3 rounded-md border border-border bg-surface p-2"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <div className="flex gap-2">
        <input
          className="min-w-0 flex-1 rounded-md border border-border bg-background px-3 py-2 text-xs outline-none transition-colors placeholder:text-muted-foreground focus:border-accent"
          onChange={(event) => onChange(event.target.value)}
          placeholder="https://example.com/article"
          type="url"
          value={value}
        />
        <Button disabled={loading || value.trim().length === 0} title={t("Save URL")} type="submit">
          <Plus className="h-4 w-4" aria-hidden="true" />
          {t("Save")}
        </Button>
      </div>
      {job ? (
        <div className={`mt-2 rounded-sm px-2 py-1 text-xs leading-5 ${jobTone}`}>
          <p className="font-medium">
            {failure ? t(failure.title) : isDeduplicated ? t("Already saved") : t("Capture job {status}", { status: t(job.status) })}
          </p>
          {failure ? <p>{t(failure.message)}</p> : null}
          {!failure && isDeduplicated ? (
            <p>{t("Opened the existing library item instead of creating a duplicate.")}</p>
          ) : null}
          {!failure && !isDeduplicated && job.lifecycleStatus ? <p>{t("Object is now {status}.", { status: t(job.lifecycleStatus) })}</p> : null}
        </div>
      ) : null}
      {error ? (
        <div className="mt-2 rounded-sm bg-red-50 dark:bg-red-950/30 px-2 py-1 text-xs leading-5 text-red-800 dark:text-red-200">
          <p className="font-medium">{t(error.title)}</p>
          <p>{t(error.message)}</p>
        </div>
      ) : null}
    </form>
  );
}
