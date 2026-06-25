import { useCallback, useEffect, useRef, useState } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { SettingsPanel, type SettingsPanelName } from "../components/settings/SettingsPanel";
import { useCheckSearchIndex } from "../hooks/commands/useCheckSearchIndex";
import { useDeleteObject } from "../hooks/commands/useDeleteObject";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { useObjectJobs } from "../hooks/commands/useObjectJobs";
import { usePing } from "../hooks/commands/usePing";
import { useRecentObjects } from "../hooks/commands/useRecentObjects";
import { useRebuildSearchIndex } from "../hooks/commands/useRebuildSearchIndex";
import { useReindexObject } from "../hooks/commands/useReindexObject";
import { useRetryBackgroundJob } from "../hooks/commands/useRetryBackgroundJob";
import { useSearchHybrid } from "../hooks/commands/useSearchHybrid";
import { useSubmitCapture } from "../hooks/commands/useSubmitCapture";
import { useTriggerAIEnrichment } from "../hooks/commands/useTriggerAIEnrichment";
import { useTriggerEvaluation } from "../hooks/commands/useTriggerEvaluation";
import { formatAIFailureReason } from "../lib/aiFailures";
import type { AppUiError } from "../lib/errors";
import { useLibraryStore } from "../store/libraryStore";
import { useSearchStore } from "../store/searchStore";
import { useUiStore } from "../store/uiStore";
import type {
  BackgroundJob,
  KnowledgeObject,
  RebuildSearchIndexResponse,
  SearchIndexHealthResponse,
} from "../types/api";

const LIBRARY_PAGE_SIZE = 30;

interface CaptureJobCompletedPayload {
  jobId?: string;
  status: "succeeded" | "failed" | "skipped" | string;
  objectId?: string;
  lifecycleStatus?: string;
  parsedDocumentId?: string;
  failureReason?: string;
}

interface AIEnrichmentCompletedPayload {
  jobId?: string;
  status: "succeeded" | "failed" | string;
  objectId?: string;
  analysisId?: string;
  failureReason?: string;
}

interface EvaluationCompletedPayload {
  objectId?: string;
  runId?: string;
}

interface SearchIndexUpdatedPayload {
  jobId?: string;
  objectId?: string;
  indexed?: boolean;
  indexedObjects?: number;
}

export function LibraryShellContainer() {
  const { route, setRoute } = useUiStore();
  const [captureUrl, setCaptureUrl] = useState("");
  const [lastCaptureJob, setLastCaptureJob] = useState<CaptureJobCompletedPayload>();
  const [hasMoreObjects, setHasMoreObjects] = useState(false);
  const [searchMaintenanceMode, setSearchMaintenanceMode] = useState<"check" | "rebuild">();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const { objects, selectedObjectId, selectedDetail, selectObject, setObjects, setSelectedDetail } = useLibraryStore();
  const { query: searchQuery, setQuery: setSearchQuery } = useSearchStore();
  const { data, error, loading, ping } = usePing();
  const {
    data: recentObjects,
    error: recentObjectsError,
    loading: recentObjectsLoading,
    loadRecentObjects,
  } = useRecentObjects();
  const {
    data: objectDetail,
    error: objectDetailError,
    loading: objectDetailLoading,
    loadObjectDetail,
    resetObjectDetail,
  } = useObjectDetail();
  const {
    error: submitCaptureError,
    loading: submitCaptureLoading,
    submitCapture,
  } = useSubmitCapture();
  const {
    error: deleteObjectError,
    loading: deleteObjectLoading,
    deleteObject,
  } = useDeleteObject();
  const {
    data: objectJobs,
    error: objectJobsError,
    loading: objectJobsLoading,
    loadObjectJobs,
    resetObjectJobs,
  } = useObjectJobs();
  const {
    error: retryJobError,
    loading: retryJobLoading,
    retryBackgroundJob,
  } = useRetryBackgroundJob();
  const {
    data: searchResults,
    error: searchError,
    loading: searchLoading,
    resetSearch,
    searchHybrid,
  } = useSearchHybrid();
  const {
    data: rebuildSearchIndexResult,
    error: rebuildSearchIndexError,
    loading: rebuildSearchIndexLoading,
    rebuildSearchIndex,
  } = useRebuildSearchIndex();
  const {
    data: searchIndexHealth,
    error: searchIndexHealthError,
    loading: searchIndexHealthLoading,
    checkSearchIndex,
  } = useCheckSearchIndex();
  const {
    data: reindexObjectResult,
    error: reindexObjectError,
    loading: reindexObjectLoading,
    reindexObject,
  } = useReindexObject();
  const {
    data: aiRunResult,
    error: triggerAIError,
    loading: triggerAILoading,
    triggerAIEnrichment,
  } = useTriggerAIEnrichment();
  const {
    error: triggerEvaluationError,
    loading: triggerEvaluationLoading,
    triggerEvaluation,
  } = useTriggerEvaluation();
  const selectedSearchResult = searchResults.find((result) => result.object.id === selectedObjectId);
  const selectedObject = objects.find((object) => object.id === selectedObjectId) ?? selectedSearchResult?.object;
  const retryableCaptureJob = findRetryableCaptureJob(objectJobs, selectedObject?.id);
  const libraryFilter =
    route.name === "library" && route.filter && route.filter !== "all"
      ? route.filter
      : undefined;

  const refreshRecentObjects = useCallback(async () => {
    const page = await loadRecentObjects({
      filterType: libraryFilter,
      limit: LIBRARY_PAGE_SIZE,
      offset: 0,
    });
    setHasMoreObjects(page.length === LIBRARY_PAGE_SIZE);
    return page;
  }, [libraryFilter, loadRecentObjects]);

  const refreshSearchResults = useCallback(() => {
    const query = searchQuery.trim();

    if (!query) {
      resetSearch();
      return Promise.resolve([]);
    }

    return searchHybrid({ query, filterType: libraryFilter, limit: 25 });
  }, [libraryFilter, resetSearch, searchHybrid, searchQuery]);

  useEffect(() => {
    void refreshRecentObjects();
  }, [refreshRecentObjects]);


  useEffect(() => {
    const query = searchQuery.trim();
    if (!query) {
      resetSearch();
      return;
    }

    const timeoutId = window.setTimeout(() => {
      void refreshSearchResults();
    }, 200);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [refreshSearchResults, resetSearch, searchQuery]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unlistenCapture: (() => void) | undefined;
    let unlistenAI: (() => void) | undefined;
    let unlistenEvaluation: (() => void) | undefined;
    let unlistenSearchRebuilt: (() => void) | undefined;
    let unlistenObjectReindexed: (() => void) | undefined;
    let disposed = false;

    void import("@tauri-apps/api/event")
      .then(async ({ listen }) => {
        const unsubscribeLibrary = await listen("library://objects-updated", () => {
          void refreshRecentObjects();
          void refreshSearchResults();
        });
        const unsubscribeCapture = await listen<CaptureJobCompletedPayload>("capture://job-completed", (event) => {
          setLastCaptureJob(event.payload);
          void refreshRecentObjects();
          void refreshSearchResults();

          const objectId = event.payload.objectId;
          if (objectId && objectId === selectedObjectId) {
            void loadObjectDetail(objectId);
            void loadObjectJobs({ objectId, limit: 10 });
          }
        });
        const unsubscribeAI = await listen<AIEnrichmentCompletedPayload>(
          "ai://enrichment-completed",
          (event) => {
            void refreshRecentObjects();
            void refreshSearchResults();

            const objectId = event.payload.objectId;
            if (objectId && objectId === selectedObjectId) {
              void loadObjectDetail(objectId);
              void loadObjectJobs({ objectId, limit: 10 });
            }
          },
        );
        const unsubscribeEvaluation = await listen<EvaluationCompletedPayload>("evaluation://completed", (event) => {
          void refreshRecentObjects();
          void refreshSearchResults();

          const objectId = event.payload.objectId;
          if (objectId && objectId === selectedObjectId) {
            void loadObjectDetail(objectId);
            void loadObjectJobs({ objectId, limit: 10 });
          }
        });
        const unsubscribeSearchRebuilt = await listen<SearchIndexUpdatedPayload>("search://index-rebuilt", () => {
          void refreshRecentObjects();
          void refreshSearchResults();
        });
        const unsubscribeObjectReindexed = await listen<SearchIndexUpdatedPayload>(
          "search://object-reindexed",
          (event) => {
            void refreshSearchResults();

            const objectId = event.payload.objectId;
            if (objectId && objectId === selectedObjectId) {
              void loadObjectDetail(objectId);
              void loadObjectJobs({ objectId, limit: 10 });
            }
          },
        );

        return {
          unsubscribeAI,
          unsubscribeCapture,
          unsubscribeEvaluation,
          unsubscribeLibrary,
          unsubscribeObjectReindexed,
          unsubscribeSearchRebuilt,
        };
      })
      .then(
        ({
          unsubscribeAI,
          unsubscribeCapture,
          unsubscribeEvaluation,
          unsubscribeLibrary,
          unsubscribeObjectReindexed,
          unsubscribeSearchRebuilt,
        }) => {
          if (disposed) {
            unsubscribeLibrary();
            unsubscribeCapture();
            unsubscribeAI();
            unsubscribeEvaluation();
            unsubscribeSearchRebuilt();
            unsubscribeObjectReindexed();
            return;
          }

          unlisten = unsubscribeLibrary;
          unlistenCapture = unsubscribeCapture;
          unlistenAI = unsubscribeAI;
          unlistenEvaluation = unsubscribeEvaluation;
          unlistenSearchRebuilt = unsubscribeSearchRebuilt;
          unlistenObjectReindexed = unsubscribeObjectReindexed;
        },
      )
      .catch(() => {
        unlisten = undefined;
        unlistenCapture = undefined;
        unlistenAI = undefined;
        unlistenEvaluation = undefined;
        unlistenSearchRebuilt = undefined;
        unlistenObjectReindexed = undefined;
      });

    return () => {
      disposed = true;
      unlisten?.();
      unlistenCapture?.();
      unlistenAI?.();
      unlistenEvaluation?.();
      unlistenSearchRebuilt?.();
      unlistenObjectReindexed?.();
    };
  }, [loadObjectDetail, loadObjectJobs, refreshRecentObjects, refreshSearchResults, selectedObjectId]);

  useEffect(() => {
    setObjects(recentObjects);
  }, [recentObjects, setObjects]);

  useEffect(() => {
    if (!selectedObjectId) {
      resetObjectDetail();
      resetObjectJobs();
      setSelectedDetail(undefined);
      return;
    }

    void loadObjectDetail(selectedObjectId);
    void loadObjectJobs({ objectId: selectedObjectId, limit: 10 });
  }, [
    loadObjectDetail,
    loadObjectJobs,
    resetObjectDetail,
    resetObjectJobs,
    selectedObjectId,
    setSelectedDetail,
  ]);

  useEffect(() => {
    setSelectedDetail(objectDetail);
  }, [objectDetail, setSelectedDetail]);

  const handleCaptureSubmit = useCallback(async () => {
    const url = captureUrl.trim();
    if (!url) {
      return;
    }

    setLastCaptureJob(undefined);
    const response = await submitCapture({
      sourceType: "url",
      sourceUrl: url,
      canonicalUrl: url,
      metadata: {},
      privacyLevel: "personal",
      permissionContext: {
        acquisitionMode: "user_action",
        userConfirmed: true,
        allowedForCloudProcessing: false,
        allowedForThirdPartyAI: false,
      },
    });

    if (!response) {
      return;
    }

    setCaptureUrl("");
    if (response.deduplicated) {
      setLastCaptureJob({
        status: "deduplicated",
        objectId: response.objectId,
        lifecycleStatus: "already saved",
      });
    }
    selectObject(response.objectId);
    await refreshRecentObjects();
  }, [captureUrl, refreshRecentObjects, selectObject, submitCapture]);

  const handleDeleteObject = useCallback(async () => {
    if (!selectedObjectId) {
      return;
    }

    const confirmed = window.confirm("Delete this item from the local library?");
    if (!confirmed) {
      return;
    }

    const response = await deleteObject({
      objectId: selectedObjectId,
      mode: "soft_delete",
    });

    if (!response) {
      return;
    }

    resetObjectDetail();
    resetObjectJobs();
    setSelectedDetail(undefined);
    const nextObjects = await refreshRecentObjects();
    await refreshSearchResults();
    setObjects(nextObjects);
  }, [
    deleteObject,
    refreshRecentObjects,
    refreshSearchResults,
    resetObjectDetail,
    resetObjectJobs,
    selectedObjectId,
    setObjects,
    setSelectedDetail,
  ]);

  const handleRetryCapture = useCallback(async () => {
    if (!retryableCaptureJob) {
      return;
    }

    const retried = await retryBackgroundJob({ jobId: retryableCaptureJob.id });
    if (!retried) {
      return;
    }

    await refreshRecentObjects();
    await refreshSearchResults();
    if (selectedObjectId) {
      await loadObjectDetail(selectedObjectId);
      await loadObjectJobs({ objectId: selectedObjectId, limit: 10 });
    }
  }, [
    loadObjectDetail,
    loadObjectJobs,
    refreshRecentObjects,
    refreshSearchResults,
    retryBackgroundJob,
    retryableCaptureJob,
    selectedObjectId,
  ]);


  const handleRunAIAnalysis = useCallback(async () => {
    if (!selectedObjectId) {
      return;
    }

    const run = await triggerAIEnrichment({ objectId: selectedObjectId });
    if (!run) {
      return;
    }

    await Promise.all([
      refreshRecentObjects(),
      refreshSearchResults(),
      loadObjectDetail(selectedObjectId),
      loadObjectJobs({ objectId: selectedObjectId, limit: 10 }),
    ]);
  }, [
    loadObjectDetail,
    loadObjectJobs,
    refreshRecentObjects,
    refreshSearchResults,
    selectedObjectId,
    triggerAIEnrichment,
  ]);

  const handleLoadMoreObjects = useCallback(async () => {
    const page = await loadRecentObjects({
      append: true,
      filterType: libraryFilter,
      limit: LIBRARY_PAGE_SIZE,
      offset: objects.length,
    });
    setHasMoreObjects(page.length === LIBRARY_PAGE_SIZE);
  }, [libraryFilter, loadRecentObjects, objects.length]);

  const handleRebuildSearchIndex = useCallback(async () => {
    setSearchMaintenanceMode("rebuild");
    const response = await rebuildSearchIndex();
    if (!response) {
      return;
    }

    await Promise.all([refreshRecentObjects(), refreshSearchResults()]);
  }, [rebuildSearchIndex, refreshRecentObjects, refreshSearchResults]);

  const handleCheckSearchIndex = useCallback(async () => {
    setSearchMaintenanceMode("check");
    await checkSearchIndex();
  }, [checkSearchIndex]);

  const handleReindexSelectedObject = useCallback(async () => {
    if (!selectedObjectId) {
      return;
    }

    const response = await reindexObject({ objectId: selectedObjectId });
    if (!response) {
      return;
    }

    await Promise.all([
      refreshRecentObjects(),
      refreshSearchResults(),
      loadObjectDetail(selectedObjectId),
      loadObjectJobs({ objectId: selectedObjectId, limit: 10 }),
    ]);
  }, [
    loadObjectDetail,
    loadObjectJobs,
    refreshRecentObjects,
    refreshSearchResults,
    reindexObject,
    selectedObjectId,
  ]);

  const handleRunEvaluation = useCallback(async () => {
    if (!selectedObjectId) {
      return;
    }

    const run = await triggerEvaluation({
      objectId: selectedObjectId,
      evaluatorType: inferEvaluatorType(selectedObject),
    });
    if (!run) {
      return;
    }

    await Promise.all([
      refreshRecentObjects(),
      refreshSearchResults(),
      loadObjectDetail(selectedObjectId),
      loadObjectJobs({ objectId: selectedObjectId, limit: 10 }),
    ]);
  }, [
    loadObjectDetail,
    loadObjectJobs,
    refreshRecentObjects,
    refreshSearchResults,
    selectedObject,
    selectedObjectId,
    triggerEvaluation,
  ]);

  if (route.name === "settings") {
    const panel = (route.panel ?? "models") as SettingsPanelName;
    return (
      <AppShell>
        <div className="grid min-h-screen grid-cols-[232px_minmax(0,1fr)]">
          <aside className="border-r border-border bg-surface">
            <Sidebar route={route} onNavigate={setRoute} />
          </aside>
          <SettingsPanel
            panel={panel}
            onPanelChange={(nextPanel) => setRoute({ name: "settings", panel: nextPanel })}
          />
        </div>
      </AppShell>
    );
  }
  return (
    <AppShell>
      <ThreePaneLayout
        sidebar={<Sidebar route={route} onNavigate={setRoute} />}
        list={
          <ObjectList
            objects={objects}
            heading={libraryHeading(route)}
            hasMore={hasMoreObjects}
            selectedObjectId={selectedObjectId}
            loading={recentObjectsLoading}
            error={recentObjectsError}
            captureValue={captureUrl}
            captureLoading={submitCaptureLoading}
            captureError={submitCaptureError}
            captureJob={lastCaptureJob}
            searchInputRef={searchInputRef}
            searchValue={searchQuery}
            searchResults={searchResults}
            searchLoading={searchLoading}
            searchError={searchError}
            searchMaintenanceLoading={rebuildSearchIndexLoading || searchIndexHealthLoading}
            searchMaintenanceError={rebuildSearchIndexError ?? searchIndexHealthError}
            searchMaintenanceMessage={searchMaintenanceMessage(
              searchMaintenanceMode,
              rebuildSearchIndexResult,
              searchIndexHealth,
            )}
            onCaptureValueChange={setCaptureUrl}
            onCaptureSubmit={handleCaptureSubmit}
            onSearchValueChange={setSearchQuery}
            onClearSearch={() => {
              setSearchQuery("");
              resetSearch();
            }}
            onCheckSearchIndex={handleCheckSearchIndex}
            onRebuildSearchIndex={handleRebuildSearchIndex}
            onLoadMore={() => {
              void handleLoadMoreObjects();
            }}
            onSelectObject={selectObject}
          />
        }
        detail={
          <ObjectDetail
            object={selectedObject}
            detail={selectedDetail}
            detailLoading={objectDetailLoading}
            detailError={objectDetailError}
            pingData={data}
            pingError={error}
            pingLoading={loading}
            deleteError={deleteObjectError}
            deleteLoading={deleteObjectLoading}
            retryJob={retryableCaptureJob}
            retryError={retryJobError ?? objectJobsError}
            retryLoading={retryJobLoading || objectJobsLoading}
            aiRunLoading={triggerAILoading}
            aiError={triggerAIError ?? aiRunFailureToError(aiRunResult)}
            searchIndexLoading={reindexObjectLoading}
            searchIndexError={reindexObjectError}
            searchIndexMessage={reindexStatusMessage(reindexObjectResult, selectedObjectId)}
            evaluationLoading={triggerEvaluationLoading}
            evaluationError={triggerEvaluationError}
            onPing={() => {
              void ping();
            }}
            onDeleteObject={handleDeleteObject}
            onRetryCapture={handleRetryCapture}
            onOpenModelSettings={() => setRoute({ name: "settings", panel: "models" })}
            onRunAIAnalysis={handleRunAIAnalysis}
            onReindexObject={handleReindexSelectedObject}
            onRunEvaluation={handleRunEvaluation}
          />
        }
      />
    </AppShell>
  );
}

function libraryHeading(route: ReturnType<typeof useUiStore.getState>["route"]) {
  if (route.name !== "library") {
    return "All";
  }
  const labels: Record<string, string> = {
    all: "All",
    article: "Articles",
    failed: "Failed",
    github_repo: "GitHub",
    inbox: "Inbox",
    prompt: "Prompts",
  };
  return labels[route.filter ?? "all"] ?? "All";
}

function reindexStatusMessage(
  result: { objectId: string; indexed: boolean } | undefined,
  selectedObjectId?: string,
) {
  if (!result || result.objectId !== selectedObjectId) {
    return undefined;
  }

  return result.indexed ? "Search index updated." : "No parsed document available for indexing.";
}

function searchMaintenanceMessage(
  mode: "check" | "rebuild" | undefined,
  rebuildResult?: RebuildSearchIndexResponse,
  health?: SearchIndexHealthResponse,
) {
  if (mode === "rebuild" && rebuildResult) {
    return `Indexed ${rebuildResult.indexedObjects} objects`;
  }

  if (mode !== "check" || !health) {
    return undefined;
  }

  if (health.healthy) {
    return `Search index healthy: ${health.actualIndexedRows}/${health.expectedIndexedObjects} rows indexed.`;
  }

  return `Search index needs rebuild: ${health.missingObjects} missing, ${health.staleObjects} stale, ${health.orphanedRows} orphaned, ${health.duplicateRows} duplicate.`;
}

function inferEvaluatorType(object?: KnowledgeObject) {
  if (!object) {
    return "auto";
  }

  if (object.type === "github_repo" || isGithubUrl(object.canonicalUrl)) {
    return "github_repo_evaluator";
  }

  return "prompt_evaluator";
}

function isGithubUrl(url?: string) {
  if (!url) {
    return false;
  }

  try {
    return new URL(url).hostname.toLowerCase().replace(/^www\./, "") === "github.com";
  } catch {
    return false;
  }
}

function aiRunFailureToError(run?: { status: string; failureReason?: string }): AppUiError | undefined {
  if (!run || run.status !== "failed") {
    return undefined;
  }

  const failure = run.failureReason
    ? formatAIFailureReason(run.failureReason)
    : undefined;

  return {
    code: failure?.code ?? "ERR_MODEL_OUTPUT_SCHEMA",
    title: failure?.title ?? "AI analysis failed",
    message: failure?.message ?? "The model provider did not return a usable analysis.",
    retryable: failure?.retryable ?? true,
    action: failure?.action ?? ("retry" as const),
  };
}

function findRetryableCaptureJob(jobs: BackgroundJob[], objectId?: string) {
  if (!objectId) {
    return undefined;
  }

  return jobs.find(
    (job) =>
      job.objectId === objectId &&
      job.type === "capture.fetch_url" &&
      ["failed", "cancelled", "blocked"].includes(job.status),
  );
}

