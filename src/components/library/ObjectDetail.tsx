import { lazy, Suspense, useState, useRef, useEffect } from "react";
import { Activity, ArrowUp, Brain, Link2, RefreshCw, Search, Trash2, PanelRight, PanelRightClose } from "lucide-react";
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
import { useI18n } from "../../i18n";

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
  libraryEmpty?: boolean;
  onPing: () => void;
  onDeleteObject: () => void;
  onRetryCapture: () => void;
  onOpenModelSettings: () => void;
  onFocusCapture?: () => void;
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
  libraryEmpty = false,
  onPing,
  onDeleteObject,
  onRetryCapture,
  onOpenModelSettings,
  onFocusCapture = noop,
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
  const { t } = useI18n();
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
    if (libraryEmpty) {
      return (
        <EmptyLibraryOnboarding
          onFocusCapture={onFocusCapture}
          onOpenModelSettings={onOpenModelSettings}
        />
      );
    }

    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t("Select an item to inspect.")}
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
  const statusText = t(formatRelativeStatus(currentObject.lifecycleStatus));

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
            title={t("Delete object")}
            className="text-muted-foreground hover:text-red-700 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-950/30 w-8 h-8 p-0"
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
          </Button>
          <Button 
            variant="ghost"
            onClick={onPing} 
            disabled={pingLoading} 
            title={t("Ping backend")}
            className="text-muted-foreground w-8 h-8 p-0"
          >
            <RefreshCw className="h-4 w-4" aria-hidden="true" />
          </Button>
          <div className="w-px h-4 bg-border mx-1" />
          <Button
            variant="ghost"
            onClick={() => setDetailSidebarCollapsed(!detailSidebarCollapsed)}
            title={detailSidebarCollapsed ? t("Show sidebar") : t("Hide sidebar")}
            className="text-muted-foreground w-8 h-8 p-0"
          >
            <PanelRight className="h-4 w-4" aria-hidden="true" />
          </Button>
          <Button
            variant="ghost"
            onClick={() => useUiStore.getState().setDetailPaneCollapsed(true)}
            title={t("Close detail pane (Ctrl+Alt+B)")}
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
              <div className="mb-4 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm leading-6 text-red-800 dark:text-red-200">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <p className="font-medium">{t(captureFailure.title)}</p>
                    <p className="mt-1">{t(captureFailure.message)}</p>
                  </div>
                  {retryJob ? (
                    <Button
                      variant="secondary"
                      onClick={onRetryCapture}
                      disabled={retryLoading}
                      title={t("Retry capture")}
                      className="shrink-0 bg-white dark:bg-surface text-red-800 dark:text-red-200 hover:bg-red-100 dark:hover:bg-red-950/50"
                    >
                      <RefreshCw className="h-4 w-4" aria-hidden="true" />
                      {t("Retry")}
                    </Button>
                  ) : null}
                </div>
              </div>
            ) : null}
            {retryError ? (
              <div className="mb-4 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm leading-6 text-red-800 dark:text-red-200">
                <p className="font-medium">{t(retryError.title)}</p>
                <p className="mt-1">{t(retryError.message)}</p>
              </div>
            ) : null}
            {deleteError ? (
              <div className="mb-4 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm leading-6 text-red-800 dark:text-red-200">
                <p className="font-medium">{t(deleteError.title)}</p>
                <p className="mt-1">{t(deleteError.message)}</p>
              </div>
            ) : null}
            {detailLoading ? (
              <p className="text-sm text-muted-foreground">{t("Loading object detail...")}</p>
            ) : null}
            {!detailLoading && detailError ? (
              <div className="rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-3 text-sm leading-6 text-red-800 dark:text-red-200">
                <p className="font-medium">{t(detailError.title)}</p>
                <p className="mt-1">{t(detailError.message)}</p>
              </div>
            ) : null}
            {!detailLoading && !detailError ? (
              <>
                <h3 className="text-lg font-semibold">{parsedDocument?.title ?? t("Parsed document preview")}</h3>
                {parsedDocument ? (
                  <Suspense fallback={<p className="mt-4 text-sm text-muted-foreground">{t("Formatting document...")}</p>}>
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
                    {t("No parsed document has been produced for this object yet.")}
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
            <h3 className="text-sm font-semibold">{t("Capture")}</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            <div className="flex items-center justify-between gap-3">
              <p className="font-medium">{t("Lifecycle")}</p>
              <span className="rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">{statusText}</span>
            </div>
            <div className="mt-2 text-muted-foreground">
              <p>{t("Snapshots {count}", { count: currentDetail?.snapshots.length ?? 0 })}</p>
              <p>{t("Parsed document {status}", { status: t(parsedDocument ? "available" : "pending") })}</p>
            </div>
            <div className="mt-3">
              <Button
                variant="secondary"
                onClick={onReindexObject}
                disabled={searchIndexLoading || !parsedDocument}
                title={t("Reindex selected object")}
              >
                <RefreshCw className={searchIndexLoading ? "h-4 w-4 animate-spin" : "h-4 w-4"} aria-hidden="true" />
                {t("Reindex")}
              </Button>
            </div>
            {searchIndexError ? (
              <div className="mt-3 rounded-md border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 p-2 text-red-800 dark:text-red-200">
                <p className="font-medium">{t(searchIndexError.title)}</p>
                <p className="mt-1">{t(searchIndexError.message)}</p>
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
          <h3 className="mt-4 text-sm font-semibold">{t("Backend IPC")}</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            {pingLoading ? <p className="text-muted-foreground">{t("Pinging backend...")}</p> : null}
            {pingData ? (
              <div>
                <p className="font-medium">{pingData.message}</p>
                <p className="mt-1 text-muted-foreground">{t("Backend {version}", { version: pingData.backendVersion })}</p>
              </div>
            ) : null}
            {pingError ? (
              <div>
                <p className="font-medium text-red-700 dark:text-red-300">{t(pingError.title)}</p>
                <p className="mt-1 text-muted-foreground">{t(pingError.message)}</p>
              </div>
            ) : null}
            {!pingLoading && !pingData && !pingError ? (
              <p className="text-muted-foreground">{t("Use Ping to validate Tauri IPC.")}</p>
            ) : null}
          </div>
        </aside>
        )}
      </div>
    </div>
  );
}

function EmptyLibraryOnboarding({
  onFocusCapture,
  onOpenModelSettings,
}: {
  onFocusCapture: () => void;
  onOpenModelSettings: () => void;
}) {
  const { t } = useI18n();
  const steps = [
    {
      icon: Link2,
      title: t("Save your first URL"),
      body: t("Paste a link into the top field to create a local item."),
    },
    {
      icon: Search,
      title: t("Find it again"),
      body: t("Search uses the local index and works before AI is configured."),
    },
    {
      icon: Brain,
      title: t("Evaluate when ready"),
      body: t("AI can be added later from Settings for analysis and evaluation."),
    },
  ];

  return (
    <div className="flex h-full min-w-0 overflow-y-auto">
      <section className="mx-auto flex w-full max-w-4xl flex-col justify-center px-10 py-12">
        <p className="text-xs font-semibold uppercase tracking-[0.14em] text-accent">
          {t("Empty library")}
        </p>
        <h2 className="mt-3 max-w-2xl text-2xl font-semibold tracking-normal text-foreground">
          {t("Start by saving one useful link.")}
        </h2>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-muted-foreground">
          {t("Node Tide is ready to save, parse, and search locally. AI is optional and can wait until there is something worth analyzing.")}
        </p>

        <div className="mt-8 grid gap-4 lg:grid-cols-3">
          {steps.map((step) => {
            const Icon = step.icon;
            return (
              <article key={step.title} className="rounded-md border border-border bg-surface p-4">
                <div className="flex h-9 w-9 items-center justify-center rounded-md bg-accent/10 text-accent">
                  <Icon className="h-4 w-4" aria-hidden="true" />
                </div>
                <h3 className="mt-4 text-sm font-semibold text-foreground">{step.title}</h3>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">{step.body}</p>
              </article>
            );
          })}
        </div>

        <div className="mt-8 flex flex-wrap items-center gap-3">
          <Button onClick={onFocusCapture} title={t("Focus save field")}>
            <ArrowUp className="h-4 w-4" aria-hidden="true" />
            {t("Paste a URL")}
          </Button>
          <Button variant="secondary" onClick={onOpenModelSettings} title={t("Configure AI (optional)")}>
            <Brain className="h-4 w-4" aria-hidden="true" />
            {t("Configure AI (optional)")}
          </Button>
        </div>
      </section>
    </div>
  );
}
