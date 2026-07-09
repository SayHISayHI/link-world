import { Search, X, CornerDownLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { formatCaptureFailureReason } from "../../lib/captureFailures";
import type { AppUiError } from "../../lib/errors";
import type { RebuildSearchIndexResponse } from "../../types/api";

interface TopBarProps {
  // Search Props
  searchValue: string;
  onSearchValueChange: (val: string) => void;
  onClearSearch: () => void;
  searchMaintenanceLoading?: boolean;
  searchRebuildStatus?: RebuildSearchIndexResponse;
  
  // Capture Props
  captureLoading: boolean;
  captureError?: AppUiError;
  captureJob?: {
    status: string;
    lifecycleStatus?: string;
    failureReason?: string;
  };
  onCaptureSubmit: (url: string) => void;
}

export function TopBar({
  searchValue,
  onSearchValueChange,
  onClearSearch,
  captureLoading,
  captureError,
  captureJob,
  onCaptureSubmit,
}: TopBarProps) {
  const [showToast, setShowToast] = useState(false);
  const [inputValue, setInputValue] = useState(searchValue);

  // Sync external search clears, unless we are holding a URL
  useEffect(() => {
    if (searchValue === "" && !/^(https?:\/\/[^\s]+)/.test(inputValue.trim())) {
      setInputValue("");
    }
  }, [searchValue, inputValue]);

  // Clear input after a successful capture
  useEffect(() => {
    if (captureJob && (captureJob.status === "succeeded" || captureJob.status === "deduplicated")) {
      setInputValue("");
    }
  }, [captureJob]);

  useEffect(() => {
    if (captureJob || captureError) {
      setShowToast(true);
      const timer = setTimeout(() => setShowToast(false), 4000);
      return () => clearTimeout(timer);
    } else {
      setShowToast(false);
    }
  }, [captureJob, captureError]);

  const searchActive = inputValue.trim().length > 0;
  const isUrl = /^(https?:\/\/[^\s]+)/.test(inputValue.trim());
  
  const failure = captureJob?.failureReason ? formatCaptureFailureReason(captureJob.failureReason) : undefined;
  const isDeduplicated = captureJob?.status === "deduplicated";
  const jobTone =
    captureJob?.status === "failed"
      ? "bg-red-50 text-red-800 border-red-200"
      : captureJob?.status === "succeeded" || isDeduplicated
        ? "bg-emerald-50 text-emerald-800 border-emerald-200"
        : "bg-muted text-muted-foreground border-border";

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && isUrl && !captureLoading) {
      e.preventDefault();
      onCaptureSubmit(inputValue.trim());
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newVal = e.target.value;
    setInputValue(newVal);
    
    if (/^(https?:\/\/[^\s]+)/.test(newVal.trim())) {
      // Intercept URL: tell system there's no search query
      onSearchValueChange("");
    } else {
      // Normal search
      onSearchValueChange(newVal);
    }
  };

  const handleClear = () => {
    setInputValue("");
    onClearSearch();
  };

  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b border-border bg-surface px-4">
      {/* Left: Brand / Title */}
      <div className="flex w-[216px] shrink-0 items-center">
        <h1 className="text-sm font-semibold tracking-normal text-foreground">Link World</h1>
      </div>

      {/* Middle: Omnibox (Search & Capture) */}
      <div className="flex min-w-0 flex-1 items-center justify-center px-6">
        <div className="relative w-full max-w-2xl">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            className="h-10 w-full rounded-md border border-border bg-background pl-9 pr-10 text-sm outline-none transition-colors focus:ring-2 focus:ring-accent focus:border-transparent"
            placeholder="Search or paste a URL to save..."
            value={inputValue}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            disabled={captureLoading}
          />
          {searchActive && !captureLoading && (
            <button
              type="button"
              onClick={handleClear}
              className="absolute right-2 top-1/2 -translate-y-1/2 rounded-sm p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
            >
              <X className="h-4 w-4" />
            </button>
          )}

          {/* Inline hint when URL is detected */}
          {isUrl && !captureJob && !captureError && !captureLoading && (
            <div className="absolute left-0 top-12 z-50 flex items-center gap-1.5 rounded-md border border-border bg-background px-3 py-1.5 text-xs text-muted-foreground shadow-sm animate-in fade-in slide-in-from-top-1">
              <CornerDownLeft className="h-3 w-3" />
              <span>Press <kbd className="rounded border bg-muted px-1 font-sans text-[10px] font-medium text-foreground">Enter</kbd> to save link</span>
            </div>
          )}

          {/* Loading State Overlay */}
          {captureLoading && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground animate-pulse">
              Saving...
            </div>
          )}

          {/* Capture Job/Error Status Popover */}
          {showToast && (captureJob || captureError) && (
            <div className="absolute left-0 top-12 z-50 w-full rounded-md border border-border bg-background p-2 shadow-md animate-in fade-in slide-in-from-top-1">
              {captureJob ? (
                <div className={`rounded-sm px-3 py-2 text-xs leading-5 border ${jobTone}`}>
                  <p className="font-medium">
                    {failure?.title ?? (isDeduplicated ? "Already saved" : `Capture job ${captureJob.status}`)}
                  </p>
                  {failure ? <p className="mt-0.5">{failure.message}</p> : null}
                  {!failure && isDeduplicated ? (
                    <p className="mt-0.5">Opened the existing library item.</p>
                  ) : null}
                  {!failure && !isDeduplicated && captureJob.lifecycleStatus ? <p className="mt-0.5">Object is now {captureJob.lifecycleStatus}.</p> : null}
                </div>
              ) : null}
              {captureError ? (
                <div className="rounded-sm border border-red-200 bg-red-50 px-3 py-2 text-xs leading-5 text-red-800">
                  <p className="font-medium">{captureError.title}</p>
                  <p className="mt-0.5">{captureError.message}</p>
                </div>
              ) : null}
            </div>
          )}
        </div>
      </div>

      {/* Right: Empty spacer to balance the layout */}
      <div className="w-[216px] shrink-0" />
    </header>
  );
}
