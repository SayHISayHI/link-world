import { lazy, Suspense, useState, useRef, useEffect } from "react";
import { Activity, RefreshCw, Trash2, PanelRight, PanelRightClose } from "lucide-react";
import { Button } from "../ui/button";
import type {
  BackgroundJob,
  KnowledgeObject,
  KnowledgeObjectDetail,
  NavigationItem,
  ObjectOrganization,
  PingResponse,
} from "../../types/api";
import type { AppUiError } from "../../lib/errors";
import { formatCaptureFailureReason } from "../../lib/captureFailures";
import { formatRelativeStatus } from "../../lib/formatting";
import { AIAnalysisPanel } from "../analysis/AIAnalysisPanel";
import { EvaluationPanel } from "../evaluation/EvaluationPanel";
import { OrganizationPanel } from "../organization/OrganizationPanel";
import { selectCurrentDisplayHints } from "./displayHints";
import { useUiStore } from "../../store/uiStore";
import { Resizer } from "../layout/Resizer";

const noop = () => undefined;

const MarkdownDocumentView = lazy(() =>
  import("./MarkdownDocumentView").then((module) => ({
    default: module.MarkdownDocumentView,
  })),
);

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
  aiRunLoading: boolean;
  aiError?: AppUiError;
  searchIndexLoading: boolean;
  searchIndexError?: AppUiError;
  searchIndexMessage?: string;
  evaluationLoading: boolean;
  evaluationError?: AppUiError;
  organization?: ObjectOrganization;
  organizationCollections?: NavigationItem[];
  organizationLoading?: boolean;
  organizationMutationLoading?: boolean;
  organizationError?: AppUiError;
  onPing: () => void;
  onDeleteObject: () => void;
  onRetryCapture: () => void;
  onOpenModelSettings: () => void;
  onRunAIAnalysis: () => void;
  onReindexObject: () => void;
  onRunEvaluation: () => void;
  onMarkFiled?: (filed: boolean) => void;
  onToggleCollection?: (collectionId: string, selected: boolean) => void;
  onAddTag?: (name: string) => void;
  onRemoveTag?: (tagId: string) => void;
  onAcceptTagSuggestion?: (suggestionId: string) => void;
  onRejectTagSuggestion?: (suggestionId: string) => void;
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
  aiRunLoading,
  aiError,
  searchIndexLoading,
  searchIndexError,
  searchIndexMessage,
  evaluationLoading,
  evaluationError,
  organization,
  organizationCollections = [],
  organizationLoading = false,
  organizationMutationLoading = false,
  organizationError,
  onPing,
  onDeleteObject,
  onRetryCapture,
  onOpenModelSettings,
  onRunAIAnalysis,
  onReindexObject,
  onRunEvaluation,
  onMarkFiled = noop,
  onToggleCollection = noop,
  onAddTag = noop,
  onRemoveTag = noop,
  onAcceptTagSuggestion = noop,
  onRejectTagSuggestion = noop,
}: ObjectDetailProps) {
  const storeWidths = useUiStore((s) => s.paneWidths);
  const setStoreWidth = useUiStore((s) => s.setPaneWidth);
  const detailSidebarCollapsed = useUiStore((s) => s.detailSidebarCollapsed);
  const setDetailSidebarCollapsed = useUiStore((s) => s.setDetailSidebarCollapsed);

  const [detailSidebarWidth, setDetailSidebarWidth] = useState(storeWidths.detailSidebar);
  const initialWidthRef = useRef(storeWidths.detailSidebar);

  useEffect(() => {
    setDetailSidebarWidth(storeWidths.detailSidebar);
  }, [storeWidths.detailSidebar]);

  const handleDragStart = () => {
    initialWidthRef.current = detailSidebarWidth;
  };

  const handleDrag = (deltaX: number) => {
    // Note: dragging the left edge to the right means a positive deltaX decreases the width
    // because the handle is on the left edge.
    // So new width = initial - deltaX
    setDetailSidebarWidth(Math.max(200, Math.min(500, initialWidthRef.current - deltaX)));
  };

  const handleDragEnd = () => {
    setStoreWidth("detailSidebar", detailSidebarWidth);
  };

  const currentDetail = detail && (!object || detail.object.id === object.id) ? detail : undefined;
  const currentObject = currentDetail?.object ?? object;

  if (!currentObject) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        Select an item to inspect.
      </div>
    );
  }

  const title = currentObject.title ?? currentObject.canonicalUrl ?? currentObject.id;
  const captureFailure = currentObject.failureReason
    ? formatCaptureFailureReason(currentObject.failureReason)
    : undefined;
  const parsedDocument = currentDetail?.parsedDocument;
  const latestAnalysis = currentDetail?.aiAnalyses[0];
  const displayHints = parsedDocument
    ? selectCurrentDisplayHints(parsedDocument.id, currentDetail?.aiAnalyses ?? [])
    : undefined;
  const latestEvaluation = currentDetail?.evaluations[0];
  const statusText = formatRelativeStatus(currentObject.lifecycleStatus);

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex h-14 items-center justify-between border-b border-border px-5">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{title}</h2>
          <p className="text-xs text-muted-foreground">
            {currentObject.type} / {statusText}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            onClick={onDeleteObject}
            disabled={deleteLoading}
            title="Delete object"
            className="text-muted-foreground hover:text-red-700 hover:bg-red-50 w-8 h-8 p-0"
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
          </Button>
          <Button 
            variant="ghost"
            onClick={onPing} 
            disabled={pingLoading} 
            title="Ping backend"
            className="text-muted-foreground w-8 h-8 p-0"
          >
            <RefreshCw className="h-4 w-4" aria-hidden="true" />
          </Button>
          <div className="w-px h-4 bg-border mx-1" />
          <Button
            variant="ghost"
            onClick={() => setDetailSidebarCollapsed(!detailSidebarCollapsed)}
            title={detailSidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
            className="text-muted-foreground w-8 h-8 p-0"
          >
            <PanelRight className="h-4 w-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            onClick={() => useUiStore.getState().setDetailPaneCollapsed(true)}
            title="Close detail pane (Ctrl+Alt+B)"
            className="text-muted-foreground hover:text-foreground w-8 h-8 p-0"
          >
            <PanelRightClose className="h-4 w-4" aria-hidden="true" />
          </Button>
        </div>
      </header>
      <div className="flex min-h-0 flex-1">
        <article className="min-w-0 flex-1 overflow-y-auto p-6">
          <div className="max-w-4xl">
            <div className="mb-4 flex items-center gap-2 text-xs text-muted-foreground">
              <Activity className="h-4 w-4" aria-hidden="true" />
              {statusText}
            </div>
            {captureFailure ? (
              <div className="mb-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm leading-6 text-red-800">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="font-medium">{captureFailure.title}</p>
                    <p className="mt-1">{captureFailure.message}</p>
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
                  <Suspense fallback={<p className="mt-4 text-sm text-muted-foreground">Formatting document...</p>}>
                    <MarkdownDocumentView
                      documentId={parsedDocument.id}
                      markdown={parsedDocument.markdown}
                      text={parsedDocument.text}
                      sourceUrl={currentObject.canonicalUrl}
                      displayHints={displayHints}
                    />
                  </Suspense>
                ) : (
                  <p className="mt-3 text-sm leading-6 text-muted-foreground">
                    No parsed document has been produced for this object yet.
                  </p>
                )}
              </>
            ) : null}
          </div>
        </article>
        
        {!detailSidebarCollapsed && (
          <aside 
            className="relative shrink-0 border-l border-border bg-background p-4 overflow-y-auto transition-[width] duration-200 ease-in-out"
            style={{ width: detailSidebarWidth }}
          >
            <Resizer 
              className="absolute left-0 top-0 bottom-0 -translate-x-1/2"
              onDragStart={handleDragStart}
              onDrag={handleDrag} 
              onDragEnd={handleDragEnd} 
            />
            <h3 className="text-sm font-semibold">Capture</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            <div className="flex items-center justify-between gap-3">
              <p className="font-medium">Lifecycle</p>
              <span className="rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">{statusText}</span>
            </div>
            <div className="mt-2 text-muted-foreground">
              <p>Snapshots {currentDetail?.snapshots.length ?? 0}</p>
              <p>Parsed document {parsedDocument ? "available" : "pending"}</p>
            </div>
            <div className="mt-3">
              <Button
                variant="secondary"
                onClick={onReindexObject}
                disabled={searchIndexLoading || !parsedDocument}
                title="Reindex selected object"
              >
                <RefreshCw className={searchIndexLoading ? "h-4 w-4 animate-spin" : "h-4 w-4"} aria-hidden="true" />
                Reindex
              </Button>
            </div>
            {searchIndexError ? (
              <div className="mt-3 rounded-md border border-red-200 bg-red-50 p-2 text-red-800">
                <p className="font-medium">{searchIndexError.title}</p>
                <p className="mt-1">{searchIndexError.message}</p>
              </div>
            ) : searchIndexMessage ? (
              <p className="mt-3 text-muted-foreground">{searchIndexMessage}</p>
            ) : null}
          </div>
          <OrganizationPanel
            organization={organization}
            collections={organizationCollections}
            loading={organizationLoading}
            mutationLoading={organizationMutationLoading}
            error={organizationError}
            onMarkFiled={onMarkFiled}
            onToggleCollection={onToggleCollection}
            onAddTag={onAddTag}
            onRemoveTag={onRemoveTag}
            onAcceptSuggestion={onAcceptTagSuggestion}
            onRejectSuggestion={onRejectTagSuggestion}
          />          <AIAnalysisPanel
            latestAnalysis={latestAnalysis}
            runLoading={aiRunLoading}
            error={aiError}
            onOpenSettings={onOpenModelSettings}
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
        )}
      </div>
    </div>
  );
}
