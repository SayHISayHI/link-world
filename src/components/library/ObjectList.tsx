import { RefreshCw } from "lucide-react";
import type { RefObject } from "react";
import type { KnowledgeObject, SearchResult } from "../../types/api";
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
  onCaptureValueChange: (value: string) => void;
  onCaptureSubmit: () => void;
  onSearchValueChange: (value: string) => void;
  onClearSearch: () => void;
  onRebuildSearchIndex: () => void;
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
  captureJob,
  searchInputRef,
  searchValue,
  searchResults,
  searchLoading,
  searchError,
  searchMaintenanceLoading,
  searchMaintenanceError,
  searchMaintenanceMessage,
  onCaptureValueChange,
  onCaptureSubmit,
  onSearchValueChange,
  onClearSearch,
  onRebuildSearchIndex,
  onSelectObject,
}: ObjectListProps) {
  const searchActive = searchValue.trim().length > 0;
  const displayObjects = searchActive ? searchResults.map((result) => result.object) : objects;
  const resultByObjectId = new Map(searchResults.map((result) => [result.object.id, result]));

  return (
    <div className="h-screen overflow-y-auto p-3">
      <div className="mb-3 px-1">
        <h1 className="text-base font-semibold">{searchActive ? "Search" : "Inbox"}</h1>
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
            className="flex h-9 shrink-0 items-center gap-1 rounded-md border border-border px-3 text-xs text-muted-foreground hover:bg-muted disabled:cursor-not-allowed disabled:opacity-60"
            disabled={searchMaintenanceLoading}
            onClick={onRebuildSearchIndex}
            title="Rebuild search index"
            type="button"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", searchMaintenanceLoading && "animate-spin")} aria-hidden="true" />
            Rebuild
          </button>
        </div>
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
          No matching objects in the local FTS index.
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
    </div>
  );
}
