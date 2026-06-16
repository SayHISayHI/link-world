import { Plus } from "lucide-react";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

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
  const jobTone =
    job?.status === "failed"
      ? "bg-red-50 text-red-800"
      : job?.status === "succeeded"
        ? "bg-emerald-50 text-emerald-800"
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
        <Button disabled={loading || value.trim().length === 0} title="Save URL" type="submit">
          <Plus className="h-4 w-4" aria-hidden="true" />
          Save
        </Button>
      </div>
      {job ? (
        <div className={`mt-2 rounded-sm px-2 py-1 text-xs leading-5 ${jobTone}`}>
          <p className="font-medium">Capture job {job.status}</p>
          {job.failureReason ? <p>{job.failureReason}</p> : null}
          {!job.failureReason && job.lifecycleStatus ? <p>Object is now {job.lifecycleStatus}.</p> : null}
        </div>
      ) : null}
      {error ? (
        <div className="mt-2 rounded-sm bg-red-50 px-2 py-1 text-xs leading-5 text-red-800">
          <p className="font-medium">{error.title}</p>
          <p>{error.message}</p>
        </div>
      ) : null}
    </form>
  );
}
