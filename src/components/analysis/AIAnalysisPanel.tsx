import { Settings, Sparkles } from "lucide-react";
import type { AIAnalysis } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

interface AIAnalysisPanelProps {
  latestAnalysis?: AIAnalysis;
  runLoading: boolean;
  error?: AppUiError;
  onOpenSettings: () => void;
  onRunAnalysis: () => void;
}

export function AIAnalysisPanel({
  latestAnalysis,
  runLoading,
  error,
  onOpenSettings,
  onRunAnalysis,
}: AIAnalysisPanelProps) {
  return (
    <section>
      <div className="mt-4 flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold">AI Analysis</h3>
        <Button variant="ghost" className="h-8 px-2 text-xs" onClick={onOpenSettings}>
          <Settings className="h-4 w-4" aria-hidden="true" />
          Models
        </Button>
      </div>
      <div className="mt-3 space-y-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
        <Button onClick={onRunAnalysis} disabled={runLoading} className="h-8 w-full text-xs">
          <Sparkles className="h-4 w-4" aria-hidden="true" />
          {runLoading ? "Running..." : "Run analysis"}
        </Button>
        {error ? (
          <div className="rounded-md border border-red-200 bg-red-50 p-2 text-red-800">
            <p className="font-medium">{error.title}</p>
            <p>{error.message}</p>
          </div>
        ) : null}
        {latestAnalysis ? (
          <div className="border-t border-border pt-3 text-muted-foreground">
            <p className="font-medium text-foreground">
              {latestAnalysis.category ?? "general_summary"}
            </p>
            <p className="mt-1">{latestAnalysis.summary ?? "Analysis exists without summary."}</p>
            {latestAnalysis.qualityScore !== undefined ? (
              <p className="mt-1">Quality {Math.round(latestAnalysis.qualityScore * 10)} / 10</p>
            ) : null}
            {latestAnalysis.trace ? (
              <p className="mt-1">
                {latestAnalysis.trace.provider} / {latestAnalysis.trace.model}
              </p>
            ) : null}
          </div>
        ) : (
          <p className="text-muted-foreground">
            No AI analysis yet. Model credentials are managed globally in Settings.
          </p>
        )}
      </div>
    </section>
  );
}
