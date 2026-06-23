import { Sparkles } from "lucide-react";
import type { AIAnalysis, ModelApiFamily, ModelProviderTestResult } from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { Button } from "../ui/button";

interface AIAnalysisPanelProps {
  latestAnalysis?: AIAnalysis;
  provider: string;
  apiFamily: ModelApiFamily;
  chatBaseUrl: string;
  chatModel: string;
  apiKey: string;
  hasApiKey: boolean;
  configLoading: boolean;
  testLoading: boolean;
  runLoading: boolean;
  testResult?: ModelProviderTestResult;
  error?: AppUiError;
  onProviderChange: (value: string) => void;
  onApiFamilyChange: (value: ModelApiFamily) => void;
  onChatBaseUrlChange: (value: string) => void;
  onChatModelChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onSaveConfig: () => void;
  onTestConfig: () => void;
  onRunAnalysis: () => void;
}

const PROVIDER_SUGGESTIONS = [
  "openai",
  "anthropic",
  "google",
  "deepseek",
  "openrouter",
  "groq",
  "xai",
  "ollama",
];

const API_FAMILY_OPTIONS: Array<{ value: ModelApiFamily; label: string }> = [
  { value: "openai_chat_completions", label: "OpenAI Chat Completions" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "anthropic_messages", label: "Anthropic Messages" },
  { value: "google_generative_ai", label: "Google Generative AI" },
  { value: "ollama", label: "Ollama Chat" },
];

export function AIAnalysisPanel({
  latestAnalysis,
  provider,
  apiFamily,
  chatBaseUrl,
  chatModel,
  apiKey,
  hasApiKey,
  configLoading,
  testLoading,
  runLoading,
  testResult,
  error,
  onProviderChange,
  onApiFamilyChange,
  onChatBaseUrlChange,
  onChatModelChange,
  onApiKeyChange,
  onSaveConfig,
  onTestConfig,
  onRunAnalysis,
}: AIAnalysisPanelProps) {
  return (
    <section>
      <h3 className="mt-4 text-sm font-semibold">AI Analysis</h3>
      <div className="mt-3 space-y-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
        <label className="block">
          <span className="font-medium">Provider</span>
          <input
            className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-accent"
            value={provider}
            onChange={(event) => onProviderChange(event.target.value)}
            placeholder="openai, anthropic, or a custom identifier"
            list="model-provider-suggestions"
          />
          <datalist id="model-provider-suggestions">
            {PROVIDER_SUGGESTIONS.map((suggestion) => (
              <option key={suggestion} value={suggestion} />
            ))}
          </datalist>
        </label>
        <label className="block">
          <span className="font-medium">API Protocol</span>
          <select
            className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-accent"
            value={apiFamily}
            onChange={(event) => onApiFamilyChange(event.target.value as ModelApiFamily)}
          >
            {API_FAMILY_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
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
            placeholder={hasApiKey ? "Configured — leave blank to keep it" : "Stored in memory only"}
            type="password"
            autoComplete="off"
          />
        </label>
        <div className="grid grid-cols-3 gap-2">
          <Button onClick={onSaveConfig} disabled={configLoading || testLoading} className="h-8 text-xs">
            Save
          </Button>
          <Button onClick={onTestConfig} disabled={testLoading || configLoading} className="h-8 text-xs" variant="secondary">
            {testLoading ? "Testing" : "Test"}
          </Button>
          <Button onClick={onRunAnalysis} disabled={runLoading || configLoading || testLoading} className="h-8 text-xs">
            <Sparkles className="h-4 w-4" aria-hidden="true" />
            Run
          </Button>
        </div>
        {testResult ? (
          <p className="rounded-md border border-emerald-200 bg-emerald-50 p-2 text-emerald-800">
            Connected to {testResult.provider} / {testResult.model} in {testResult.latencyMs} ms.
          </p>
        ) : null}
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
