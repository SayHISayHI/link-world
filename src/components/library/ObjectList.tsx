import { RefreshCw, XCircle, PanelRightOpen } from "lucide-react";
import type { RefObject } from "react";
import { formatRelativeStatus } from "../../lib/formatting";
import { cn } from "../../lib/cn";
import { Button } from "../ui/button";
import { useUiStore } from "../../store/uiStore";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObject, SearchResult } from "../../types/api";
import { ObjectListItem } from "./ObjectListItem";

interface ObjectListProps {
  objects: KnowledgeObject[];
  heading?: string;
  hasMore?: boolean;
  selectedObjectId?: string;
  loading: boolean;
  error?: AppUiError;
  searchResults?: SearchResult[];
  searchActive?: boolean;
  searchLoading?: boolean;
  searchError?: AppUiError;
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
  searchLoading,
  searchError,
  onLoadMore,
  onSelectObject,
}: ObjectListProps) {
  const detailPaneCollapsed = useUiStore((s) => s.detailPaneCollapsed);
  const setDetailPaneCollapsed = useUiStore((s) => s.setDetailPaneCollapsed);

  const displayObjects = searchActive && searchResults ? searchResults.map((result) => result.object) : objects;
  const resultByObjectId = searchResults ? new Map(searchResults.map((result) => [result.object.id, result])) : new Map();

  const handleSelectObject = (objectId: string) => {
    setDetailPaneCollapsed(false);
    onSelectObject(objectId);
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="flex h-14 shrink-0 items-center justify-between gap-3 px-5">
        <h1 className="min-w-0 truncate text-base font-semibold">
          {searchActive ? "Search" : heading}
        </h1>
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

      {searchActive && !searchLoading && !searchError && searchResults?.length === 0 ? (
        <div className="mx-4 my-2 rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          <p className="font-medium text-foreground">No matching objects</p>
          <p className="mt-1">
            No local search results found. Try broader terms or clear the current filter.
          </p>
        </div>
      ) : null}

      {!searchActive && !loading && !error && objects.length === 0 ? (
        <div className="rounded-md border border-dashed border-border bg-surface p-4 text-xs leading-5 text-muted-foreground">
          No captured objects yet. The local database is ready for the first browser-extension or manual capture.
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
      {!searchActive && hasMore ? (
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
