import { Activity, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "../ui/button";
import type {
  BackgroundJob,
  KnowledgeObject,
  KnowledgeObjectDetail,
  PingResponse,
} from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { formatRelativeStatus } from "../../lib/formatting";
import { AIAnalysisPanel } from "../analysis/AIAnalysisPanel";
import { EvaluationPanel } from "../evaluation/EvaluationPanel";

interface ObjectDetailProps {
  object?: KnowledgeObject;
  detail?: KnowledgeObjectDetail;
  detailLoading: boolean;
  detailError?: AppUiError;
  pingData?: PingResponse;
  pingError?: AppUiError;
  pingLoading: boolean;
  deleteError?: AppUiError;
  deleteLoading: boolean;
  retryJob?: BackgroundJob;
  retryError?: AppUiError;
  retryLoading: boolean;
  aiChatBaseUrl: string;
  aiChatModel: string;
  aiApiKey: string;
  aiConfigLoading: boolean;
  aiRunLoading: boolean;
  aiError?: AppUiError;
  evaluationLoading: boolean;
  evaluationError?: AppUiError;
  onPing: () => void;
  onDeleteObject: () => void;
  onRetryCapture: () => void;
  onAIChatBaseUrlChange: (value: string) => void;
  onAIChatModelChange: (value: string) => void;
  onAIApiKeyChange: (value: string) => void;
  onSaveAIConfig: () => void;
  onRunAIAnalysis: () => void;
  onRunEvaluation: () => void;
}

export function ObjectDetail({
  object,
  detail,
  detailLoading,
  detailError,
  pingData,
  pingError,
  pingLoading,
  deleteError,
  deleteLoading,
  retryJob,
  retryError,
  retryLoading,
  aiChatBaseUrl,
  aiChatModel,
  aiApiKey,
  aiConfigLoading,
  aiRunLoading,
  aiError,
  evaluationLoading,
  evaluationError,
  onPing,
  onDeleteObject,
  onRetryCapture,
  onAIChatBaseUrlChange,
  onAIChatModelChange,
  onAIApiKeyChange,
  onSaveAIConfig,
  onRunAIAnalysis,
  onRunEvaluation,
}: ObjectDetailProps) {
  if (!object) {
    return (
      <div className="flex h-screen items-center justify-center text-sm text-muted-foreground">
        Select an item to inspect.
      </div>
    );
  }

  const title = object.title ?? object.canonicalUrl ?? object.id;
  const parsedDocument = detail?.parsedDocument;
  const latestAnalysis = detail?.aiAnalyses[0];
  const latestEvaluation = detail?.evaluations[0];
  const statusText = formatRelativeStatus(object.lifecycleStatus);

  return (
    <div className="flex h-screen min-w-0 flex-col">
      <header className="flex h-14 items-center justify-between border-b border-border px-5">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          <p className="text-xs text-muted-foreground">
            {object.type} / {statusText}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            variant="ghost"
            onClick={onDeleteObject}
            disabled={deleteLoading}
            title="Delete object"
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
            Delete
          </Button>
          <Button onClick={onPing} disabled={pingLoading} title="Ping backend">
            <RefreshCw className="h-4 w-4" aria-hidden="true" />
            Ping
          </Button>
        </div>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_320px]">
        <article className="overflow-y-auto p-6">
          <div className="max-w-3xl">
            <div className="mb-4 flex items-center gap-2 text-xs text-muted-foreground">
              <Activity className="h-4 w-4" aria-hidden="true" />
              {statusText}
            </div>
            {object.failureReason ? (
              <div className="mb-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm leading-6 text-red-800">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="font-medium">Capture failed</p>
                    <p className="mt-1">{object.failureReason}</p>
                  </div>
                  {retryJob ? (
                    <Button
                      variant="secondary"
                      onClick={onRetryCapture}
                      disabled={retryLoading}
                      title="Retry capture"
                      className="shrink-0 bg-white text-red-800 hover:bg-red-100"
                    >
                      <RefreshCw className="h-4 w-4" aria-hidden="true" />
                      Retry
                    </Button>
                  ) : null}
                </div>
              </div>
            ) : null}
            {retryError ? (
              <div className="mb-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm leading-6 text-red-800">
                <p className="font-medium">{retryError.title}</p>
                <p className="mt-1">{retryError.message}</p>
              </div>
            ) : null}
            {deleteError ? (
              <div className="mb-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm leading-6 text-red-800">
                <p className="font-medium">{deleteError.title}</p>
                <p className="mt-1">{deleteError.message}</p>
              </div>
            ) : null}
            {detailLoading ? (
              <p className="text-sm text-muted-foreground">Loading object detail...</p>
            ) : null}
            {!detailLoading && detailError ? (
              <div className="rounded-md border border-red-200 bg-red-50 p-3 text-sm leading-6 text-red-800">
                <p className="font-medium">{detailError.title}</p>
                <p className="mt-1">{detailError.message}</p>
              </div>
            ) : null}
            {!detailLoading && !detailError ? (
              <>
                <h3 className="text-lg font-semibold">{parsedDocument?.title ?? "Parsed document preview"}</h3>
                {parsedDocument ? (
                  <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-foreground">{parsedDocument.text}</p>
                ) : (
                  <p className="mt-3 text-sm leading-6 text-muted-foreground">
                    No parsed document has been produced for this object yet.
                  </p>
                )}
              </>
            ) : null}
          </div>
        </article>
        <aside className="border-l border-border bg-background p-4">
          <h3 className="text-sm font-semibold">Capture</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            <div className="flex items-center justify-between gap-3">
              <p className="font-medium">Lifecycle</p>
              <span className="rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">{statusText}</span>
            </div>
            <div className="mt-2 text-muted-foreground">
              <p>Snapshots {detail?.snapshots.length ?? 0}</p>
              <p>Parsed document {parsedDocument ? "available" : "pending"}</p>
            </div>
          </div>
          <AIAnalysisPanel
            latestAnalysis={latestAnalysis}
            chatBaseUrl={aiChatBaseUrl}
            chatModel={aiChatModel}
            apiKey={aiApiKey}
            configLoading={aiConfigLoading}
            runLoading={aiRunLoading}
            error={aiError}
            onChatBaseUrlChange={onAIChatBaseUrlChange}
            onChatModelChange={onAIChatModelChange}
            onApiKeyChange={onAIApiKeyChange}
            onSaveConfig={onSaveAIConfig}
            onRunAnalysis={onRunAIAnalysis}
          />
          <EvaluationPanel
            latestEvaluation={latestEvaluation}
            loading={evaluationLoading}
            error={evaluationError}
            onRunEvaluation={onRunEvaluation}
          />
          <h3 className="mt-4 text-sm font-semibold">Backend IPC</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            {pingLoading ? <p className="text-muted-foreground">Pinging backend...</p> : null}
            {pingData ? (
              <div>
                <p className="font-medium">{pingData.message}</p>
                <p className="mt-1 text-muted-foreground">Backend {pingData.backendVersion}</p>
              </div>
            ) : null}
            {pingError ? (
              <div>
                <p className="font-medium text-red-700">{pingError.title}</p>
                <p className="mt-1 text-muted-foreground">{pingError.message}</p>
              </div>
            ) : null}
            {!pingLoading && !pingData && !pingError ? (
              <p className="text-muted-foreground">Use Ping to validate Tauri IPC.</p>
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}
