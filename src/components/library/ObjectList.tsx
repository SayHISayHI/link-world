import { CheckCircle2, RefreshCw, XCircle, PanelRightOpen } from "lucide-react";
import { formatRelativeStatus } from "../../lib/formatting";
import { cn } from "../../lib/cn";
import { Button } from "../ui/button";
import { useUiStore } from "../../store/uiStore";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObject, RebuildSearchIndexResponse, SearchResult } from "../../types/api";

interface ObjectListProps {
  objects: KnowledgeObject[];
  heading?: string;
  hasMore?: boolean;
  selectedObjectId?: string;
  loading: boolean;
  error?: AppUiError;
  searchResults?: SearchResult[];
  searchActive?: boolean;
  searchValue?: string;
  searchLoading?: boolean;
  searchError?: AppUiError;
  searchMaintenanceLoading?: boolean;
  searchMaintenanceError?: AppUiError;
  searchMaintenanceMessage?: string;
  searchRebuildStatus?: RebuildSearchIndexResponse;
  onCancelSearchIndexRebuild?: () => void;
  onCheckSearchIndex?: () => void;
  onRebuildSearchIndex?: () => void;
  onOpenModelSettings?: () => void;
  onLoadMore: () => void;
  onSelectObject: (objectId: string) => void;
}

export function ObjectList({
  objects,
  heading,
  hasMore,
  selectedObjectId,
  loading,
  error,
  searchResults,
  searchActive,
  searchValue = "",
  searchLoading,
  searchError,
  searchMaintenanceLoading,
  searchMaintenanceError,
  searchMaintenanceMessage,
  searchRebuildStatus,
  onCancelSearchIndexRebuild = noop,
  onCheckSearchIndex = noop,
  onRebuildSearchIndex = noop,
  onOpenModelSettings = noop,
  onLoadMore,
  onSelectObject,
}: ObjectListProps) {
  const detailPaneCollapsed = useUiStore((s) => s.detailPaneCollapsed);
  const setDetailPaneCollapsed = useUiStore((s) => s.setDetailPaneCollapsed);

  const activeSearch = searchActive ?? searchValue.trim().length > 0;
  const displayObjects = activeSearch && searchResults ? searchResults.map((result) => result.object) : objects;
  const resultByObjectId = searchResults ? new Map(searchResults.map((result) => [result.object.id, result])) : new Map();
  const rebuildRunning =
    searchRebuildStatus?.status === "queued" || searchRebuildStatus?.status === "running";
  const rebuildProgress = Math.round(searchRebuildStatus?.progressPercent ?? 0);
  const showMaintenancePanel =
    Boolean(searchRebuildStatus) || Boolean(searchMaintenanceError) || Boolean(searchMaintenanceMessage);

  const handleSelectObject = (objectId: string) => {
    setDetailPaneCollapsed(false);
    onSelectObject(objectId);
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="flex h-14 shrink-0 items-center justify-between gap-3 px-5">
        <h1 className="min-w-0 truncate text-base font-semibold">
          {activeSearch ? "Search" : heading}
        </h1>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            onClick={onCheckSearchIndex}
            disabled={searchMaintenanceLoading}
            title="Check search index"
            aria-label="Check search index"
            className="text-muted-foreground hover:text-foreground w-8 h-8 p-0"
          >
            <CheckCircle2 className="h-4 w-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            onClick={onRebuildSearchIndex}
            disabled={searchMaintenanceLoading || rebuildRunning}
            title="Rebuild search index"
            aria-label="Rebuild search index"
            className="text-muted-foreground hover:text-foreground w-8 h-8 p-0"
          >
            <RefreshCw className={cn("h-4 w-4", searchMaintenanceLoading && "animate-spin")} aria-hidden="true" />
          </Button>
          {rebuildRunning && searchRebuildStatus?.cancellable ? (
            <Button
              variant="ghost"
              onClick={onCancelSearchIndexRebuild}
              title="Cancel search index rebuild"
              aria-label="Cancel search index rebuild"
              className="text-red-600 hover:bg-red-50 hover:text-red-700 w-8 h-8 p-0"
            >
              <XCircle className="h-4 w-4" aria-hidden="true" />
            </Button>
          ) : null}
          {detailPaneCollapsed && (
            <Button
              variant="ghost"
              onClick={() => setDetailPaneCollapsed(false)}
              title="Open detail pane (Ctrl+Alt+B)"
              className="text-muted-foreground hover:text-foreground w-8 h-8 p-0"
            >
              <PanelRightOpen className="h-4 w-4" aria-hidden="true" />
            </Button>
          )}
        </div>
      </div>

      {showMaintenancePanel ? (
        <div className="mx-4 my-2 rounded-md border border-border bg-surface p-3 text-xs leading-5">
          {searchRebuildStatus ? (
            <div>
              <div className="flex items-center justify-between gap-3">
                <span className="min-w-0 text-muted-foreground">
                  {formatRebuildStatus(searchRebuildStatus)}
                </span>
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
              {rebuildRunning && searchRebuildStatus.cancellable ? (
                <button
                  className="mt-3 inline-flex h-7 items-center gap-1 rounded-md border border-red-200 px-2 text-[11px] font-medium text-red-700 hover:bg-red-50"
                  onClick={onCancelSearchIndexRebuild}
                  title="Cancel search index rebuild"
                  type="button"
                >
                  <XCircle className="h-3.5 w-3.5" aria-hidden="true" />
                  Cancel
                </button>
              ) : null}
              {rebuildRunning && !searchRebuildStatus.cancellable ? (
                <p className="mt-2 text-[11px] leading-4 text-muted-foreground">
                  Finalizing is atomic and cannot be cancelled without risking a partially swapped index.
                </p>
              ) : null}
            </div>
          ) : null}
          {searchMaintenanceError ? (
            <div className={searchRebuildStatus ? "mt-3 border-t border-border pt-3 text-red-800" : "text-red-800"}>
              <p className="font-medium">{searchMaintenanceError.title}</p>
              <p className="mt-1">{searchMaintenanceError.message}</p>
            </div>
          ) : searchMaintenanceMessage ? (
            <p className={searchRebuildStatus ? "mt-3 border-t border-border pt-3 text-muted-foreground" : "text-muted-foreground"}>
              {searchMaintenanceMessage}
            </p>
          ) : null}
        </div>
      ) : null}

      {searchLoading ? (
        <div className="mx-4 my-2 rounded-md border border-border bg-surface p-3 text-xs text-muted-foreground">
          Searching local index...
        </div>
      ) : null}

      {!searchLoading && searchError ? (
        <div className="mx-4 my-2 rounded-md border border-red-200 bg-red-50 p-3 text-xs leading-5 text-red-800">
          <p className="font-medium">{searchError.title}</p>
          <p className="mt-1">{searchError.message}</p>
          <div className="mt-2 flex flex-wrap gap-2">
            <button
              className="rounded-md border border-red-200 px-2 py-1 text-[11px] font-medium hover:bg-red-100"
              disabled={searchMaintenanceLoading}
              onClick={onCheckSearchIndex}
              type="button"
            >
              Check index
            </button>
            <button
              className="rounded-md border border-red-200 px-2 py-1 text-[11px] font-medium hover:bg-red-100"
              disabled={searchMaintenanceLoading || rebuildRunning}
              onClick={onRebuildSearchIndex}
              type="button"
            >
              Rebuild index
            </button>
          </div>
        </div>
      ) : null}

      {!activeSearch && loading ? (
        <div className="mx-4 my-2 rounded-md border border-border bg-surface p-3 text-xs text-muted-foreground">
          Loading local library...
        </div>
      ) : null}

      {!activeSearch && !loading && error ? (
        <div className="mx-4 my-2 rounded-md border border-red-200 bg-red-50 p-3 text-xs leading-5 text-red-800">
          <p className="font-medium">{error.title}</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      {activeSearch && !searchLoading && !searchError && searchResults?.length === 0 ? (
        <div className="mx-4 my-2 rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          <p className="font-medium text-foreground">No matching objects</p>
          <p className="mt-1">
            {searchValue.trim()
              ? `No local FTS result matched "${searchValue.trim()}". Try broader terms, clear the current filter, or check the index if this content should already be parsed.`
              : "No local search results found. Try broader terms or clear the current filter."}
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              className="rounded-md border border-border px-2 py-1 text-[11px] font-medium text-foreground hover:bg-muted disabled:opacity-60"
              disabled={searchMaintenanceLoading}
              onClick={onCheckSearchIndex}
              type="button"
            >
              Check index
            </button>
            <button
              className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[11px] font-medium text-foreground hover:bg-muted disabled:opacity-60"
              disabled={searchMaintenanceLoading || rebuildRunning}
              onClick={onRebuildSearchIndex}
              type="button"
            >
              <RefreshCw className={cn("h-3.5 w-3.5", searchMaintenanceLoading && "animate-spin")} aria-hidden="true" />
              Rebuild index
            </button>
          </div>
        </div>
      ) : null}

      {!activeSearch && !loading && !error && objects.length === 0 ? (
        <div className="mx-4 my-2 rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          <p className="text-sm font-semibold text-foreground">Your first useful loop</p>
          <p className="mt-1">AI is optional. Saving and searching work locally without a model.</p>
          <ol className="mt-3 space-y-2" aria-label="Getting started">
            <li><span className="font-medium text-foreground">1. Save a URL.</span> Paste it into the bar above and press Enter.</li>
            <li><span className="font-medium text-foreground">2. Search your library.</span> Find it again by title or body text.</li>
            <li><span className="font-medium text-foreground">3. Run an Evaluation.</span> Open the result and evaluate it when you are ready.</li>
          </ol>
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-border pt-3">
            <p>No captured objects yet. The local database is ready for your first capture.</p>
            <button
              className="rounded-md border border-border px-2.5 py-1.5 text-[11px] font-medium text-foreground hover:bg-muted"
              onClick={onOpenModelSettings}
              type="button"
            >
              Configure AI (optional)
            </button>
          </div>
        </div>
      ) : null}

      <div className="flex flex-col border-t border-border">
        {displayObjects.map((object) => {
          const result = resultByObjectId.get(object.id);

          return (
            <button
              key={object.id}
              className={cn(
                "relative w-full border-b border-border bg-surface px-4 py-3 text-left transition-colors hover:bg-muted/60",
                selectedObjectId === object.id && "bg-accent/[0.08] before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:bg-accent"
              )}
              onClick={() => handleSelectObject(object.id)}
              type="button"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <h3 className="truncate text-sm font-medium leading-snug text-foreground/95">
                    {object.title ?? object.canonicalUrl ?? object.id}
                  </h3>
                  {result?.snippet ? (
                    <p className="mt-1.5 line-clamp-2 text-[13px] leading-relaxed text-muted-foreground/80">
                      {result.snippet}
                    </p>
                  ) : object.canonicalUrl || object.sourcePlatform ? (
                    <p className="mt-1.5 line-clamp-1 text-[13px] leading-relaxed text-muted-foreground/80">
                      {object.sourcePlatform ? `${object.sourcePlatform} · ` : ""}
                      {object.canonicalUrl ?? object.type}
                    </p>
                  ) : null}
                  {result?.matchedFields.length ? (
                    <p className="mt-2 text-[11px] text-accent/80">
                      Matched: {result.matchedFields.join(", ")}
                    </p>
                  ) : null}
                </div>
                <span className="mt-0.5 shrink-0 rounded border border-border/60 bg-muted/30 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground/70">
                  {formatRelativeStatus(object.lifecycleStatus)}
                </span>
              </div>
            </button>
          );
        })}
      </div>
      {!activeSearch && hasMore ? (
        <div className="p-4">
          <button
            className="h-9 w-full rounded-md border border-border text-xs text-muted-foreground hover:bg-muted disabled:opacity-60"
            disabled={loading}
            onClick={onLoadMore}
            type="button"
          >
            {loading ? "Loading..." : "Load more"}
          </button>
        </div>
      ) : null}
    </div>
  );
}

function noop() {
  return undefined;
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
