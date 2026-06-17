import { Sparkles } from "lucide-react";
import type { AIAnalysis } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

interface AIAnalysisPanelProps {
  latestAnalysis?: AIAnalysis;
  chatBaseUrl: string;
  chatModel: string;
  apiKey: string;
  configLoading: boolean;
  runLoading: boolean;
  error?: AppUiError;
  onChatBaseUrlChange: (value: string) => void;
  onChatModelChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onSaveConfig: () => void;
  onRunAnalysis: () => void;
}

export function AIAnalysisPanel({
  latestAnalysis,
  chatBaseUrl,
  chatModel,
  apiKey,
  configLoading,
  runLoading,
  error,
  onChatBaseUrlChange,
  onChatModelChange,
  onApiKeyChange,
  onSaveConfig,
  onRunAnalysis,
}: AIAnalysisPanelProps) {
  return (
    <section>
      <h3 className="mt-4 text-sm font-semibold">AI Analysis</h3>
      <div className="mt-3 space-y-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
        <label className="block">
          <span className="font-medium">Chat Base URL</span>
          <input
            className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-accent"
            value={chatBaseUrl}
            onChange={(event) => onChatBaseUrlChange(event.target.value)}
            placeholder="https://api.openai.com/v1"
          />
        </label>
        <label className="block">
          <span className="font-medium">Chat Model</span>
          <input
            className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-accent"
            value={chatModel}
            onChange={(event) => onChatModelChange(event.target.value)}
            placeholder="gpt-4.1-mini"
          />
        </label>
        <label className="block">
          <span className="font-medium">API Key</span>
          <input
            className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-accent"
            value={apiKey}
            onChange={(event) => onApiKeyChange(event.target.value)}
            placeholder="Stored in memory only"
            type="password"
          />
        </label>
        <div className="grid grid-cols-2 gap-2">
          <Button onClick={onSaveConfig} disabled={configLoading} className="h-8 text-xs">
            Save
          </Button>
          <Button onClick={onRunAnalysis} disabled={runLoading} className="h-8 text-xs">
            <Sparkles className="h-4 w-4" aria-hidden="true" />
            Run
          </Button>
        </div>
        {error ? (
          <div className="rounded-md border border-red-200 bg-red-50 p-2 text-red-800">
            <p className="font-medium">{error.title}</p>
            <p>{error.message}</p>
          </div>
        ) : null}
        {latestAnalysis ? (
          <div className="border-t border-border pt-3 text-muted-foreground">
            <p className="font-medium text-foreground">{latestAnalysis.category ?? "general_summary"}</p>
            <p className="mt-1">{latestAnalysis.summary ?? "Analysis exists without summary."}</p>
            {latestAnalysis.qualityScore !== undefined ? (
              <p className="mt-1">Quality {latestAnalysis.qualityScore}</p>
            ) : null}
            {latestAnalysis.trace ? (
              <p className="mt-1">
                {latestAnalysis.trace.provider} / {latestAnalysis.trace.model}
              </p>
            ) : null}
          </div>
        ) : (
          <p className="text-muted-foreground">No AI analysis yet.</p>
        )}
      </div>
    </section>
  );
}
