import type { KnowledgeObject } from "../../types/api";
import { formatRelativeStatus } from "../../lib/formatting";
import { cn } from "../../lib/cn";
import type { AppUiError } from "../../lib/errors";
import { CaptureBar } from "./CaptureBar";

interface ObjectListProps {
  objects: KnowledgeObject[];
  selectedObjectId?: string;
  loading: boolean;
  error?: AppUiError;
  captureValue: string;
  captureLoading: boolean;
  captureError?: AppUiError;
  onCaptureValueChange: (value: string) => void;
  onCaptureSubmit: () => void;
  onSelectObject: (objectId: string) => void;
}

export function ObjectList({
  objects,
  selectedObjectId,
  loading,
  error,
  captureValue,
  captureLoading,
  captureError,
  onCaptureValueChange,
  onCaptureSubmit,
  onSelectObject,
}: ObjectListProps) {
  return (
    <div className="h-screen overflow-y-auto p-3">
      <div className="mb-3 px-1">
        <h1 className="text-base font-semibold">Inbox</h1>
        <p className="mt-1 text-xs text-muted-foreground">Captured items ready for processing.</p>
      </div>

      <CaptureBar
        error={captureError}
        loading={captureLoading}
        onChange={onCaptureValueChange}
        onSubmit={onCaptureSubmit}
        value={captureValue}
      />

      {loading ? (
        <div className="rounded-md border border-border bg-surface p-3 text-xs text-muted-foreground">
          Loading local library...
        </div>
      ) : null}

      {!loading && error ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-3 text-xs leading-5 text-red-800">
          <p className="font-medium">{error.title}</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {!loading && !error && objects.length === 0 ? (
        <div className="rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          No captured objects yet. The local database is ready for the first browser-extension or manual capture.
        </div>
      ) : null}

      <div className="space-y-2">
        {objects.map((object) => (
          <button
            key={object.id}
            className={cn(
              "w-full rounded-md border border-border bg-surface p-3 text-left transition-colors hover:bg-muted",
              selectedObjectId === object.id && "border-accent bg-accent/5",
            )}
            onClick={() => onSelectObject(object.id)}
            type="button"
          >
            <div className="flex items-center justify-between gap-3">
              <div className="truncate text-sm font-medium">{object.title ?? object.canonicalUrl ?? object.id}</div>
              <span className="shrink-0 rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">
                {formatRelativeStatus(object.lifecycleStatus)}
              </span>
            </div>
            {object.canonicalUrl || object.sourcePlatform ? (
              <p className="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">
                {object.sourcePlatform ? `${object.sourcePlatform} - ` : ""}
                {object.canonicalUrl ?? object.type}
              </p>
            ) : null}
          </button>
        ))}
      </div>
    </div>
  );
}
