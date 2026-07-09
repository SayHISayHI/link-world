import { Search, X, CornerDownLeft, CheckCircle2, AlertCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { useUiStore } from "../../store/uiStore";
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

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && isUrl && !captureLoading) {
      e.preventDefault();
      onCaptureSubmit(inputValue.trim());
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setShowToast(false);
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

  const { paneWidths } = useUiStore();

  return (
    <header className="flex h-14 shrink-0 items-center justify-between border-b border-border/60 bg-surface pr-4">
      {/* Left: Brand / Title */}
      <div 
        className="flex shrink-0 items-center px-4" 
        style={{ width: paneWidths.sidebar }}
      >
        <h1 className="text-sm font-semibold tracking-normal text-foreground truncate mr-2">Link World</h1>
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
          {/* Status and Actions Container */}
          <div className="absolute right-2 top-1/2 flex -translate-y-1/2 items-center gap-2">
            {captureLoading ? (
              <div className="flex items-center gap-1.5 pr-2 text-xs text-muted-foreground animate-pulse">
                <span className="h-3.5 w-3.5 rounded-full border-2 border-muted-foreground border-t-transparent animate-spin" />
                <span>Saving...</span>
              </div>
            ) : showToast && (captureJob || captureError) ? (
              <div className={`group relative flex cursor-default items-center gap-1.5 pr-2 text-xs animate-in fade-in zoom-in-95 ${captureError || captureJob?.status === 'failed' ? 'text-red-500' : 'text-accent'}`}>
                {captureError || captureJob?.status === 'failed' ? (
                  <>
                    <AlertCircle className="h-4 w-4" />
                    <span className="font-medium">{captureError?.title ?? "Failed"}</span>
                    {/* Error Tooltip */}
                    <div className="absolute right-0 top-full mt-2 hidden w-64 rounded-md border border-border bg-surface p-3 text-xs text-foreground shadow-lg group-hover:block z-50">
                      {captureError?.message ?? failure?.message ?? "An unknown error occurred."}
                    </div>
                  </>
                ) : (
                  <>
                    <CheckCircle2 className="h-4 w-4" />
                    <span className="font-medium">{isDeduplicated ? "Already saved" : "Saved"}</span>
                  </>
                )}
              </div>
            ) : isUrl ? (
              <div className="flex items-center gap-1.5 pr-2 text-xs text-muted-foreground animate-in fade-in">
                <span>Press <kbd className="rounded border border-border bg-muted px-1 font-sans text-[10px] font-medium text-foreground">Enter</kbd></span>
                <CornerDownLeft className="h-3 w-3" />
              </div>
            ) : searchActive ? (
              <button
                type="button"
                onClick={handleClear}
                className="rounded-sm p-1 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
              >
                <X className="h-4 w-4" />
              </button>
            ) : null}
          </div>
        </div>
      </div>

      {/* Right: Empty spacer to balance the layout */}
      <div className="w-[216px] shrink-0" />
    </header>
  );
}
