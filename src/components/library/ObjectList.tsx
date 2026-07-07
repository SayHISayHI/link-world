import { RefreshCw, XCircle } from "lucide-react";
import type { RefObject } from "react";
import type {
  KnowledgeObject,
  KnowledgeObjectType,
  RebuildSearchIndexResponse,
  SearchResult,
} from "../../types/api";
import { formatRelativeStatus } from "../../lib/formatting";
import { cn } from "../../lib/cn";
import type { AppUiError } from "../../lib/errors";
import { CaptureBar } from "./CaptureBar";

interface ObjectListProps {
  objects: KnowledgeObject[];
  heading: string;
  hasMore: boolean;
  selectedObjectId?: string;
  loading: boolean;
  error?: AppUiError;
  captureValue: string;
  captureLoading: boolean;
  captureError?: AppUiError;
  captureJob?: {
    status: string;
    lifecycleStatus?: string;
    failureReason?: string;
  };
  searchInputRef?: RefObject<HTMLInputElement>;
  searchValue: string;
  searchResults: SearchResult[];
  searchLoading: boolean;
  searchError?: AppUiError;
  searchMaintenanceLoading: boolean;
  searchMaintenanceError?: AppUiError;
  searchMaintenanceMessage?: string;
  searchRebuildStatus?: RebuildSearchIndexResponse;
  objectTypeFilter?: KnowledgeObjectType;
  onCaptureValueChange: (value: string) => void;
  onCaptureSubmit: () => void;
  onSearchValueChange: (value: string) => void;
  onClearSearch: () => void;
  onCancelSearchIndexRebuild: () => void;
  onCheckSearchIndex: () => void;
  onRebuildSearchIndex: () => void;
  onLoadMore: () => void;
  onSelectObject: (objectId: string) => void;
  onObjectTypeFilterChange?: (value?: KnowledgeObjectType) => void;
}

export function ObjectList({
  objects,
  heading,
  hasMore,
  selectedObjectId,
  loading,
  error,
  captureValue,
  captureLoading,
  captureError,
  captureJob,
  searchInputRef,
  searchValue,
  searchResults,
  searchLoading,
  searchError,
  searchMaintenanceLoading,
  searchMaintenanceError,
  searchMaintenanceMessage,
  searchRebuildStatus,
  objectTypeFilter,
  onCaptureValueChange,
  onCaptureSubmit,
  onSearchValueChange,
  onClearSearch,
  onCancelSearchIndexRebuild,
  onCheckSearchIndex,
  onRebuildSearchIndex,
  onLoadMore,
  onSelectObject,
  onObjectTypeFilterChange = () => undefined,
}: ObjectListProps) {
  const searchActive = searchValue.trim().length > 0;
  const displayObjects = searchActive ? searchResults.map((result) => result.object) : objects;
  const resultByObjectId = new Map(searchResults.map((result) => [result.object.id, result]));
  const rebuildRunning =
    searchRebuildStatus?.status === "queued" || searchRebuildStatus?.status === "running";
  const rebuildProgress = Math.round(searchRebuildStatus?.progressPercent ?? 0);

  return (
    <div className="h-full overflow-y-auto p-3">
      <div className="mb-3 px-1">
        <div className="flex items-center justify-between gap-3">
          <h1 className="min-w-0 truncate text-base font-semibold">
            {searchActive ? "Search" : heading}
          </h1>
          <select
            value={objectTypeFilter ?? ""}
            onChange={(event) =>
              onObjectTypeFilterChange(
                (event.target.value || undefined) as KnowledgeObjectType | undefined,
              )
            }
            className="h-8 max-w-32 rounded-sm border border-border bg-surface px-2 text-xs text-muted-foreground outline-none focus:border-accent"
            aria-label="Filter content type"
          >
            <option value="">All types</option>
            <option value="article">Articles</option>
            <option value="github_repo">GitHub repos</option>
            <option value="prompt">Prompts</option>
            <option value="note">Notes</option>
          </select>
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          {searchActive ? "FTS results from parsed text and AI summaries." : "Captured items ready for processing."}
        </p>
      </div>
      <div className="mb-3 rounded-md border border-border bg-surface p-2">
        <div className="flex items-center gap-2">
          <input
            ref={searchInputRef}
            className="h-9 min-w-0 flex-1 rounded-md border border-border bg-background px-3 text-sm outline-none focus:ring-2 focus:ring-accent"
            onChange={(event) => onSearchValueChange(event.target.value)}
            placeholder="Search title, content, AI summary"
            value={searchValue}
          />
          {searchActive ? (
            <button
              className="h-9 shrink-0 rounded-md border border-border px-3 text-xs text-muted-foreground hover:bg-muted"
              onClick={onClearSearch}
              type="button"
            >
              Clear
            </button>
          ) : null}
          <button
            className="h-9 shrink-0 rounded-md border border-border px-3 text-xs text-muted-foreground hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
            disabled={searchMaintenanceLoading}
            onClick={onCheckSearchIndex}
            title="Check search index"
            type="button"
          >
            Check
          </button>
          <button
            className="flex h-9 shrink-0 items-center gap-1 rounded-md border border-border px-3 text-xs text-muted-foreground hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
            disabled={searchMaintenanceLoading || rebuildRunning}
            onClick={onRebuildSearchIndex}
            title="Rebuild search index"
            type="button"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", searchMaintenanceLoading && "animate-spin")} aria-hidden="true" />
            Rebuild
          </button>
          {rebuildRunning && searchRebuildStatus?.cancellable ? (
            <button
              className="flex h-9 shrink-0 items-center gap-1 rounded-md border border-red-200 px-3 text-xs text-red-700 hover:bg-red-50"
              onClick={onCancelSearchIndexRebuild}
              title="Cancel search index rebuild"
              type="button"
            >
              <XCircle className="h-3.5 w-3.5" aria-hidden="true" />
              Cancel
            </button>
          ) : null}
        </div>
        {searchRebuildStatus ? (
          <div className="mt-2 rounded-md border border-border bg-background p-2">
            <div className="flex items-center justify-between gap-3 text-xs">
              <span className="text-muted-foreground">{formatRebuildStatus(searchRebuildStatus)}</span>
              <span className="shrink-0 font-medium text-foreground">{rebuildProgress}%</span>
            </div>
            <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className={cn(
                  "h-full rounded-full transition-all",
                  searchRebuildStatus.status === "failed" || searchRebuildStatus.status === "cancelled"
                    ? "bg-red-400"
                    : "bg-accent",
                )}
                style={{ width: `${Math.min(Math.max(rebuildProgress, 0), 100)}%` }}
              />
            </div>
            {rebuildRunning && !searchRebuildStatus.cancellable ? (
              <p className="mt-2 text-[11px] leading-4 text-muted-foreground">
                Finalizing is atomic and cannot be cancelled without risking a partially swapped index.
              </p>
            ) : null}
          </div>
        ) : null}
        {searchMaintenanceError ? (
          <div className="mt-2 rounded-md border border-red-200 bg-red-50 p-2 text-xs leading-5 text-red-800">
            <p className="font-medium">{searchMaintenanceError.title}</p>
            <p className="mt-1">{searchMaintenanceError.message}</p>
          </div>
        ) : searchMaintenanceMessage ? (
          <p className="mt-2 text-xs text-muted-foreground">{searchMaintenanceMessage}</p>
        ) : null}
      </div>

      <CaptureBar
        error={captureError}
        job={captureJob}
        loading={captureLoading}
        onChange={onCaptureValueChange}
        onSubmit={onCaptureSubmit}
        value={captureValue}
      />

      {searchLoading ? (
        <div className="rounded-md border border-border bg-surface p-3 text-xs text-muted-foreground">
          Searching local index...
        </div>
      ) : null}

      {!searchLoading && searchError ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-3 text-xs leading-5 text-red-800">
          <p className="font-medium">{searchError.title}</p>
          <p className="mt-1">{searchError.message}</p>
          <div className="mt-2 flex flex-wrap gap-2">
            <button
              className="rounded-md border border-red-200 px-2 py-1 text-[11px] font-medium hover:bg-red-100"
              onClick={onCheckSearchIndex}
              type="button"
            >
              Check index
            </button>
            <button
              className="rounded-md border border-red-200 px-2 py-1 text-[11px] font-medium hover:bg-red-100"
              disabled={rebuildRunning}
              onClick={onRebuildSearchIndex}
              type="button"
            >
              Rebuild index
            </button>
          </div>
        </div>
      ) : null}

      {!searchActive && loading ? (
        <div className="rounded-md border border-border bg-surface p-3 text-xs text-muted-foreground">
          Loading local library...
        </div>
      ) : null}

      {!searchActive && !loading && error ? (
        <div className="rounded-md border border-red-200 bg-red-50 p-3 text-xs leading-5 text-red-800">
          <p className="font-medium">{error.title}</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {searchActive && !searchLoading && !searchError && searchResults.length === 0 ? (
        <div className="rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          <p className="font-medium text-foreground">No matching objects</p>
          <p className="mt-1">
            No local FTS result matched "{searchValue.trim()}". Try broader terms, clear the current filter, or check
            the index if this content should already be parsed.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              className="rounded-md border border-border px-2 py-1 text-[11px] font-medium text-foreground hover:bg-muted"
              onClick={onCheckSearchIndex}
              type="button"
            >
              Check index
            </button>
            <button
              className="rounded-md border border-border px-2 py-1 text-[11px] font-medium text-foreground hover:bg-muted disabled:opacity-60"
              disabled={rebuildRunning}
              onClick={onRebuildSearchIndex}
              type="button"
            >
              Rebuild index
            </button>
          </div>
        </div>
      ) : null}

      {!searchActive && !loading && !error && objects.length === 0 ? (
        <div className="rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          No captured objects yet. The local database is ready for the first browser-extension or manual capture.
        </div>
      ) : null}

      <div className="space-y-2">
        {displayObjects.map((object) => {
          const result = resultByObjectId.get(object.id);

          return (
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
              {result?.snippet ? (
                <p className="mt-2 line-clamp-3 text-xs leading-5 text-muted-foreground">{result.snippet}</p>
              ) : object.canonicalUrl || object.sourcePlatform ? (
                <p className="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">
                  {object.sourcePlatform ? `${object.sourcePlatform} - ` : ""}
                  {object.canonicalUrl ?? object.type}
                </p>
              ) : null}
              {result?.matchedFields.length ? (
                <p className="mt-2 text-[11px] text-muted-foreground">
                  Matched {result.matchedFields.join(", ")}
                </p>
              ) : null}
            </button>
          );
        })}
      </div>
      {!searchActive && hasMore ? (
        <button
          className="mt-3 h-9 w-full rounded-md border border-border text-xs text-muted-foreground hover:bg-muted disabled:opacity-60"
          disabled={loading}
          onClick={onLoadMore}
          type="button"
        >
          {loading ? "Loading..." : "Load more"}
        </button>
      ) : null}
    </div>
  );
}

function formatRebuildStatus(status: RebuildSearchIndexResponse) {
  if (status.status === "succeeded") {
    return `Search index rebuilt: ${status.indexedObjects}/${status.expectedObjects} objects indexed.`;
  }

  if (status.status === "cancelled") {
    return "Search index rebuild cancelled. Existing search index was preserved.";
  }

  if (status.status === "failed") {
    return status.failureReason
      ? `Search index rebuild failed: ${status.failureReason}`
      : "Search index rebuild failed.";
  }

  const stageLabel: Record<string, string> = {
    finalizing: "Finalizing atomic index swap",
    indexing: "Indexing searchable objects",
    preparing: "Preparing staging index",
    queued: "Starting rebuild",
  };

  return `${stageLabel[status.stage] ?? "Rebuilding search index"}: ${status.indexedObjects}/${status.expectedObjects} objects indexed.`;
}
