import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { TopBar } from "../components/layout/TopBar";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { SettingsPanel, type SettingsPanelName } from "../components/settings/SettingsPanel";
import { useCheckSearchIndex } from "../hooks/commands/useCheckSearchIndex";
import { useDeleteObject } from "../hooks/commands/useDeleteObject";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { useObjectJobs } from "../hooks/commands/useObjectJobs";
import { usePing } from "../hooks/commands/usePing";
import { useLibraryNavigation } from "../hooks/commands/useLibraryNavigation";
import { useLibraryObjects } from "../hooks/commands/useLibraryObjects";
import { useObjectOrganization } from "../hooks/commands/useObjectOrganization";
import { useOrganizationMutations } from "../hooks/commands/useOrganizationMutations";
import { useRebuildSearchIndex } from "../hooks/commands/useRebuildSearchIndex";
import { useReindexObject } from "../hooks/commands/useReindexObject";
import { useRetryBackgroundJob } from "../hooks/commands/useRetryBackgroundJob";
import { useRetryEvaluation } from "../hooks/commands/useRetryEvaluation";
import { useSearchLibrary } from "../hooks/commands/useSearchLibrary";
import { useSubmitCapture } from "../hooks/commands/useSubmitCapture";
import { useTriggerAIEnrichment } from "../hooks/commands/useTriggerAIEnrichment";
import { useTriggerEvaluation } from "../hooks/commands/useTriggerEvaluation";
import { allLibraryView, emptyLibraryFilters } from "../app/routes";
import { formatAIFailureReason } from "../lib/aiFailures";
import {
  subscribeToLibraryEvents,
  type CaptureJobCompletedPayload,
} from "../lib/libraryEvents";
import type { AppUiError } from "../lib/errors";
import { useLibraryStore } from "../store/libraryStore";
import { useSearchStore } from "../store/searchStore";
import { useUiStore } from "../store/uiStore";
import type {
  BackgroundJob,
  KnowledgeObject,
  LibraryNavigation,
  LibraryQuery,
  NavigationItem,
  RebuildSearchIndexResponse,
  SearchIndexHealthResponse,
} from "../types/api";

const LIBRARY_PAGE_SIZE = 30;

export function LibraryShellContainer() {
  const route = useUiStore((s) => s.route);
  const setRoute = useUiStore((s) => s.setRoute);
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const paneWidths = useUiStore((s) => s.paneWidths);
  const [lastCaptureJob, setLastCaptureJob] = useState<CaptureJobCompletedPayload>();
  const [searchMaintenanceMode, setSearchMaintenanceMode] = useState<"check" | "rebuild">();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const lastCompletedSearchRebuildJobRef = useRef<string>();
  const { objects, selectedObjectId, selectedDetail, selectObject, setObjects, setSelectedDetail } = useLibraryStore();
  const { query: searchQuery, setQuery: setSearchQuery } = useSearchStore();
  const { data, error, loading, ping } = usePing();
  const {
    data: libraryObjects,
    nextCursor,
    error: libraryObjectsError,
    loading: libraryObjectsLoading,
    loadLibraryObjects,
  } = useLibraryObjects();
  const {
    data: navigation,
    error: navigationError,
    loading: navigationLoading,
    loadNavigation,
  } = useLibraryNavigation();
  const {
    data: objectOrganization,
    error: objectOrganizationError,
    loading: objectOrganizationLoading,
    loadObjectOrganization,
    resetObjectOrganization,
  } = useObjectOrganization();
  const organizationMutations = useOrganizationMutations();
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
    searchLibrary,
  } = useSearchLibrary();
  const {
    data: rebuildSearchIndexResult,
    error: rebuildSearchIndexError,
    loading: rebuildSearchIndexLoading,
    cancelSearchIndexRebuild,
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
    resetTriggerEvaluation,
  } = useTriggerEvaluation();
  const {
    error: retryEvaluationError,
    loading: retryEvaluationLoading,
    retryEvaluation,
    resetRetryEvaluation,
  } = useRetryEvaluation();
  const selectedSearchResult = searchResults.find((result) => result.object.id === selectedObjectId);
  const selectedObject = objects.find((object) => object.id === selectedObjectId) ?? selectedSearchResult?.object;
  const retryableCaptureJob = findRetryableCaptureJob(objectJobs, selectedObject?.id);
  const sidebarWidth = sidebarCollapsed ? 64 : paneWidths.sidebar;
  const libraryQuery = useMemo<LibraryQuery>(
    () => ({
      view: route.name === "library" ? (route.view ?? allLibraryView) : allLibraryView,
      filters: route.name === "library" ? (route.filters ?? emptyLibraryFilters) : emptyLibraryFilters,
      limit: LIBRARY_PAGE_SIZE,
    }),
    [route],
  );

  const refreshRecentObjects = useCallback(async () => {
    const page = await loadLibraryObjects({ query: { ...libraryQuery, cursor: undefined } });
    return page?.items ?? [];
  }, [libraryQuery, loadLibraryObjects]);

  const refreshSearchResults = useCallback(() => {
    const query = searchQuery.trim();

    if (!query) {
      resetSearch();
      return Promise.resolve([]);
    }

    return searchLibrary({
      query,
      limit: 25,
      libraryQuery: { ...libraryQuery, cursor: undefined },
    });
  }, [libraryQuery, resetSearch, searchLibrary, searchQuery]);
  const eventActionsRef = useRef({
    loadObjectDetail,
    loadObjectJobs,
    loadObjectOrganization,
    loadNavigation,
    refreshRecentObjects,
    refreshSearchResults,
  });
  eventActionsRef.current = {
    loadObjectDetail,
    loadObjectJobs,
    loadObjectOrganization,
    loadNavigation,
    refreshRecentObjects,
    refreshSearchResults,
  };

  useEffect(() => {
    void refreshRecentObjects();
    void loadNavigation();
  }, [loadNavigation, refreshRecentObjects]);


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
    if (!rebuildSearchIndexResult || rebuildSearchIndexResult.status !== "succeeded") {
      return;
    }

    if (lastCompletedSearchRebuildJobRef.current === rebuildSearchIndexResult.jobId) {
      return;
    }

    lastCompletedSearchRebuildJobRef.current = rebuildSearchIndexResult.jobId;
    void Promise.all([refreshRecentObjects(), refreshSearchResults()]);
  }, [rebuildSearchIndexResult, refreshRecentObjects, refreshSearchResults]);

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
    let disposed = false;
    let unsubscribe: (() => void) | undefined;

    const refreshLibrary = () => {
      const actions = eventActionsRef.current;
      void actions.refreshRecentObjects();
      void actions.refreshSearchResults();
      void actions.loadNavigation();
      const selectedId = useLibraryStore.getState().selectedObjectId;
      if (selectedId) {
        void actions.loadObjectOrganization(selectedId);
      }
    };
    const refreshSelectedObject = (objectId?: string) => {
      if (!objectId || objectId !== useLibraryStore.getState().selectedObjectId) {
        return;
      }

      const actions = eventActionsRef.current;
      void actions.loadObjectDetail(objectId);
      void actions.loadObjectJobs({ objectId, limit: 10 });
    };

    void subscribeToLibraryEvents({
      onObjectsUpdated: refreshLibrary,
      onCaptureJobCompleted: (payload) => {
        setLastCaptureJob(payload);
        refreshLibrary();
        refreshSelectedObject(payload.objectId);
      },
      onAIEnrichmentCompleted: (payload) => {
        refreshLibrary();
        refreshSelectedObject(payload.objectId);
      },
      onEvaluationCompleted: (payload) => {
        refreshLibrary();
        refreshSelectedObject(payload.objectId);
      },
      onSearchIndexRebuilt: refreshLibrary,
      onObjectReindexed: (payload) => {
        void eventActionsRef.current.refreshSearchResults();
        refreshSelectedObject(payload.objectId);
      },
    })
      .then((nextUnsubscribe) => {
        if (disposed) {
          nextUnsubscribe();
          return;
        }
        unsubscribe = nextUnsubscribe;
      })
      .catch((error) => {
        console.error("Failed to subscribe to backend library events.", error);
      });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  useEffect(() => {
    setObjects(libraryObjects);
  }, [libraryObjects, setObjects]);

  useEffect(() => {
    if (!selectedObjectId) {
      resetObjectDetail();
      resetObjectJobs();
      resetObjectOrganization();
      setSelectedDetail(undefined);
      return;
    }

    void loadObjectDetail(selectedObjectId);
    void loadObjectJobs({ objectId: selectedObjectId, limit: 10 });
    void loadObjectOrganization(selectedObjectId);
  }, [
    loadObjectDetail,
    loadObjectJobs,
    loadObjectOrganization,
    resetObjectDetail,
    resetObjectOrganization,
    resetObjectJobs,
    selectedObjectId,
    setSelectedDetail,
  ]);

  useEffect(() => {
    setSelectedDetail(objectDetail);
  }, [objectDetail, setSelectedDetail]);

  const handleCaptureSubmit = useCallback(async (submittedUrl: string) => {
    const url = submittedUrl.trim();
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

    setSearchQuery("");
    if (response.deduplicated) {
      setLastCaptureJob({
        status: "deduplicated",
        objectId: response.objectId,
        lifecycleStatus: "already saved",
      });
    }
    selectObject(response.objectId);
  }, [setSearchQuery, selectObject, submitCapture]);

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
    loadObjectOrganization,
    loadNavigation,
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
    loadObjectOrganization,
    loadNavigation,
    refreshRecentObjects,
    refreshSearchResults,
    selectedObjectId,
    triggerAIEnrichment,
  ]);

  const handleLoadMoreObjects = useCallback(async () => {
    if (!nextCursor) {
      return;
    }
    await loadLibraryObjects({
      append: true,
      query: { ...libraryQuery, cursor: nextCursor },
    });
  }, [libraryQuery, loadLibraryObjects, nextCursor]);
  const handleRebuildSearchIndex = useCallback(async () => {
    setSearchMaintenanceMode("rebuild");
    await rebuildSearchIndex();
  }, [rebuildSearchIndex]);

  const handleCancelSearchIndexRebuild = useCallback(async () => {
    const jobId = rebuildSearchIndexResult?.jobId;
    if (!jobId) {
      return;
    }

    setSearchMaintenanceMode("rebuild");
    await cancelSearchIndexRebuild(jobId);
  }, [cancelSearchIndexRebuild, rebuildSearchIndexResult?.jobId]);

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
    loadObjectOrganization,
    loadNavigation,
    refreshRecentObjects,
    refreshSearchResults,
    reindexObject,
    selectedObjectId,
  ]);

  const handleRunEvaluation = useCallback(async () => {
    if (!selectedObjectId) {
      return;
    }

    const latestEvaluation = selectedDetail?.evaluations[0];
    let run;
    if (latestEvaluation?.status === "failed") {
      resetTriggerEvaluation();
      run = await retryEvaluation({ runId: latestEvaluation.id });
    } else {
      resetRetryEvaluation();
      run = await triggerEvaluation({
        objectId: selectedObjectId,
        evaluatorType: inferEvaluatorType(selectedObject),
      });
    }
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
    loadObjectOrganization,
    loadNavigation,
    refreshRecentObjects,
    refreshSearchResults,
    resetRetryEvaluation,
    resetTriggerEvaluation,
    retryEvaluation,
    selectedDetail,
    selectedObject,
    selectedObjectId,
    triggerEvaluation,
  ]);

  const refreshOrganizationUi = useCallback(async () => {
    const tasks: Array<Promise<unknown>> = [loadNavigation(), refreshRecentObjects()];
    if (searchQuery.trim()) {
      tasks.push(refreshSearchResults());
    }
    if (selectedObjectId) {
      tasks.push(loadObjectOrganization(selectedObjectId));
    }
    await Promise.all(tasks);
  }, [
    loadNavigation,
    loadObjectOrganization,
    refreshRecentObjects,
    refreshSearchResults,
    searchQuery,
    selectedObjectId,
  ]);

  const handleCreateCollection = useCallback(
    async (name: string) => {
      const created = await organizationMutations.createCollection({
        name,
        iconKey: "folder",
      });
      if (created) {
        await loadNavigation();
      }
    },
    [loadNavigation, organizationMutations],
  );

  const handleCreateSmartView = useCallback(async () => {
    const name = window.prompt("Name this smart view")?.trim();
    if (!name) {
      return;
    }
    const created = await organizationMutations.createSmartView({
      name,
      rule: {
        schemaVersion: 1,
        objectTypes: libraryQuery.filters.objectTypes,
        tagIds: libraryQuery.filters.tagIds,
        minimumQuality: libraryQuery.filters.qualityMin,
      },
    });
    if (created) {
      await loadNavigation();
    }
  }, [libraryQuery.filters, loadNavigation, organizationMutations]);
  const handleRenameCollection = useCallback(
    async (item: NavigationItem) => {
      const nextName = window.prompt("Rename collection", item.label)?.trim();
      if (!nextName || nextName === item.label || item.revision === undefined) {
        return;
      }
      const updated = await organizationMutations.updateCollection({
        collectionId: item.id,
        name: nextName,
        expectedRevision: item.revision,
      });
      if (updated) {
        await loadNavigation();
      }
    },
    [loadNavigation, organizationMutations],
  );

  const handleArchiveCollection = useCallback(
    async (item: NavigationItem) => {
      if (!window.confirm("Archive " + item.label + "? Saved items will remain in the library.")) {
        return;
      }
      const archived = await organizationMutations.archiveCollection(item.id);
      if (!archived) {
        return;
      }
      if (route.name === "library" && route.view?.kind === "collection" && route.view.id === item.id) {
        setRoute({ name: "library", view: allLibraryView, filters: emptyLibraryFilters });
      }
      await loadNavigation();
    },
    [loadNavigation, organizationMutations, route, setRoute],
  );

  const handleMarkFiled = useCallback(
    async (filed: boolean) => {
      if (!selectedObjectId) return;
      const updated = await organizationMutations.markObjectTriaged(selectedObjectId, filed);
      if (updated) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi, selectedObjectId],
  );

  const handleToggleCollection = useCallback(
    async (collectionId: string, selected: boolean) => {
      if (!selectedObjectId) return;
      const updated = selected
        ? await organizationMutations.addObjectToCollection(selectedObjectId, collectionId)
        : await organizationMutations.removeObjectFromCollection(selectedObjectId, collectionId);
      if (updated) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi, selectedObjectId],
  );

  const handleAddTag = useCallback(
    async (name: string) => {
      if (!selectedObjectId) return;
      const tag = await organizationMutations.addUserTag(selectedObjectId, name);
      if (tag) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi, selectedObjectId],
  );

  const handleRemoveTag = useCallback(
    async (tagId: string) => {
      if (!selectedObjectId) return;
      const removed = await organizationMutations.removeObjectTag(selectedObjectId, tagId);
      if (removed) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi, selectedObjectId],
  );

  const handleAcceptTagSuggestion = useCallback(
    async (suggestionId: string) => {
      const tag = await organizationMutations.acceptTagSuggestion(suggestionId);
      if (tag) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi],
  );

  const handleRejectTagSuggestion = useCallback(
    async (suggestionId: string) => {
      const rejected = await organizationMutations.rejectTagSuggestion(suggestionId);
      if (rejected) await refreshOrganizationUi();
    },
    [organizationMutations, refreshOrganizationUi],
  );
  if (route.name === "settings") {
    const panel = (route.panel ?? "models") as SettingsPanelName;
    return (
      <AppShell>
        <div
          className="grid min-h-screen"
          style={{ gridTemplateColumns: `${sidebarWidth}px minmax(0, 1fr)` }}
        >
          <aside className="border-r border-border bg-surface">
            <Sidebar
              route={route}
              navigation={navigation}
              loading={navigationLoading}
              mutationLoading={organizationMutations.loading}
              error={navigationError ?? organizationMutations.error}
              onNavigate={setRoute}
              onCreateCollection={handleCreateCollection}
              onCreateSmartView={handleCreateSmartView}
              onRenameCollection={handleRenameCollection}
              onArchiveCollection={handleArchiveCollection}
            />
          </aside>
          <SettingsPanel
            panel={panel}
            onPanelChange={(nextPanel) => setRoute({ name: "settings", panel: nextPanel })}
            onOpenObject={(objectId) => {
              selectObject(objectId);
              setRoute({ name: "library", view: allLibraryView, filters: emptyLibraryFilters });
            }}
          />
        </div>
      </AppShell>
    );
  }
  return (
    <AppShell>
      <ThreePaneLayout
        topBar={
          <TopBar
            searchValue={searchQuery}
            onSearchValueChange={setSearchQuery}
            onClearSearch={() => {
              setSearchQuery("");
              resetSearch();
            }}
            searchInputRef={searchInputRef}
            captureLoading={submitCaptureLoading}
            captureError={submitCaptureError}
            captureJob={lastCaptureJob}
            onCaptureSubmit={handleCaptureSubmit}
          />
        }
        sidebar={<Sidebar
              route={route}
              navigation={navigation}
              loading={navigationLoading}
              mutationLoading={organizationMutations.loading}
              error={navigationError ?? organizationMutations.error}
              onNavigate={setRoute}
              onCreateCollection={handleCreateCollection}
              onCreateSmartView={handleCreateSmartView}
              onRenameCollection={handleRenameCollection}
              onArchiveCollection={handleArchiveCollection}
            />}
        list={
          <ObjectList
            objects={objects}
            heading={libraryHeading(route, navigation)}
            hasMore={Boolean(nextCursor)}
            selectedObjectId={selectedObjectId}
            loading={libraryObjectsLoading}
            error={libraryObjectsError}
            searchResults={searchResults}
            searchActive={searchQuery.trim().length > 0}
            searchValue={searchQuery}
            searchLoading={searchLoading}
            searchError={searchError}
            searchMaintenanceLoading={rebuildSearchIndexLoading || searchIndexHealthLoading}
            searchMaintenanceError={rebuildSearchIndexError ?? searchIndexHealthError}
            searchMaintenanceMessage={searchMaintenanceMessage(
              searchMaintenanceMode,
              rebuildSearchIndexResult,
              searchIndexHealth,
            )}
            searchRebuildStatus={rebuildSearchIndexResult}
            onCancelSearchIndexRebuild={handleCancelSearchIndexRebuild}
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
            evaluationLoading={triggerEvaluationLoading || retryEvaluationLoading}
            evaluationError={triggerEvaluationError ?? retryEvaluationError}
            organization={objectOrganization}
            organizationCollections={navigation?.collections ?? []}
            organizationLoading={objectOrganizationLoading}
            organizationMutationLoading={organizationMutations.loading}
            organizationError={objectOrganizationError ?? organizationMutations.error}
            onPing={() => {
              void ping();
            }}
            onDeleteObject={handleDeleteObject}
            onRetryCapture={handleRetryCapture}
            onOpenModelSettings={() => setRoute({ name: "settings", panel: "models" })}
            onRunAIAnalysis={handleRunAIAnalysis}
            onReindexObject={handleReindexSelectedObject}
            onRunEvaluation={handleRunEvaluation}
            onMarkFiled={handleMarkFiled}
            onToggleCollection={handleToggleCollection}
            onAddTag={handleAddTag}
            onRemoveTag={handleRemoveTag}
            onAcceptTagSuggestion={handleAcceptTagSuggestion}
            onRejectTagSuggestion={handleRejectTagSuggestion}
          />
        }
      />
    </AppShell>
  );
}

function libraryHeading(
  route: ReturnType<typeof useUiStore.getState>["route"],
  navigation?: LibraryNavigation,
) {
  if (route.name !== "library") {
    return "All";
  }
  const view = route.view ?? allLibraryView;
  const items = [
    ...(navigation?.systemViews ?? []),
    ...(navigation?.collections ?? []),
    ...(navigation?.topics ?? []),
    ...(navigation?.smartViews ?? []),
  ];
  return items.find((item) => item.kind === view.kind && item.id === view.id)?.label ?? "Library";
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
    return undefined;
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
