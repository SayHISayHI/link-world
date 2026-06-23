import { useCallback, useEffect, useRef, useState } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { useDeleteObject } from "../hooks/commands/useDeleteObject";
import { useGetModelProviderConfig } from "../hooks/commands/useGetModelProviderConfig";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { useObjectJobs } from "../hooks/commands/useObjectJobs";
import { usePing } from "../hooks/commands/usePing";
import { useRecentObjects } from "../hooks/commands/useRecentObjects";
import { useRebuildSearchIndex } from "../hooks/commands/useRebuildSearchIndex";
import { useReindexObject } from "../hooks/commands/useReindexObject";
import { useRetryBackgroundJob } from "../hooks/commands/useRetryBackgroundJob";
import { useSearchHybrid } from "../hooks/commands/useSearchHybrid";
import { useSubmitCapture } from "../hooks/commands/useSubmitCapture";
import { useTestModelProviderConfig } from "../hooks/commands/useTestModelProviderConfig";
import { useTriggerAIEnrichment } from "../hooks/commands/useTriggerAIEnrichment";
import { useTriggerEvaluation } from "../hooks/commands/useTriggerEvaluation";
import { useUpdateModelProviderConfig } from "../hooks/commands/useUpdateModelProviderConfig";
import type { AppUiError } from "../lib/errors";
import { useLibraryStore } from "../store/libraryStore";
import { useSearchStore } from "../store/searchStore";
import type { BackgroundJob, KnowledgeObject, ModelApiFamily, ModelProviderConfig } from "../types/api";

interface ProviderPreset {
  apiFamily: ModelApiFamily;
  chatBaseUrl: string;
  chatModel: string;
}

const MODEL_PROVIDER_PRESETS: Record<string, ProviderPreset> = {
  openai: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.openai.com/v1",
    chatModel: "gpt-4.1-mini",
  },
  anthropic: {
    apiFamily: "anthropic_messages",
    chatBaseUrl: "https://api.anthropic.com/v1",
    chatModel: "claude-sonnet-4-5",
  },
  google: {
    apiFamily: "google_generative_ai",
    chatBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
    chatModel: "gemini-2.5-flash",
  },
  deepseek: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.deepseek.com",
    chatModel: "deepseek-chat",
  },
  openrouter: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://openrouter.ai/api/v1",
    chatModel: "openai/gpt-4.1-mini",
  },
  groq: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.groq.com/openai/v1",
    chatModel: "llama-3.3-70b-versatile",
  },
  xai: {
    apiFamily: "openai_chat_completions",
    chatBaseUrl: "https://api.x.ai/v1",
    chatModel: "grok-3-mini",
  },
  ollama: {
    apiFamily: "ollama",
    chatBaseUrl: "http://127.0.0.1:11434",
    chatModel: "llama3.2",
  },
};

interface CaptureJobCompletedPayload {
  jobId: string;
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
  const [captureUrl, setCaptureUrl] = useState("");
  const [lastCaptureJob, setLastCaptureJob] = useState<CaptureJobCompletedPayload>();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const aiConfigDirtyRef = useRef(false);
  const [aiProvider, setAIProvider] = useState("openai");
  const [aiApiFamily, setAIApiFamily] = useState<ModelApiFamily>("openai_chat_completions");
  const [aiChatBaseUrl, setAIChatBaseUrl] = useState("https://api.openai.com/v1");
  const [aiChatModel, setAIChatModel] = useState("gpt-4.1-mini");
  const [aiApiKey, setAIApiKey] = useState("");
  const [aiHasApiKey, setAIHasApiKey] = useState(false);
  const [testedAIConfigFingerprint, setTestedAIConfigFingerprint] = useState<string>();
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
    data: reindexObjectResult,
    error: reindexObjectError,
    loading: reindexObjectLoading,
    reindexObject,
  } = useReindexObject();
  const {
    data: storedModelConfig,
    error: getModelConfigError,
    loading: getModelConfigLoading,
    getModelProviderConfig,
  } = useGetModelProviderConfig();
  const {
    error: updateModelConfigError,
    loading: updateModelConfigLoading,
    updateModelProviderConfig,
  } = useUpdateModelProviderConfig();
  const {
    data: testModelConfigResult,
    error: testModelConfigError,
    loading: testModelConfigLoading,
    testModelProviderConfig,
  } = useTestModelProviderConfig();
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
  const aiConfig = createModelProviderConfig(
    aiProvider,
    aiApiFamily,
    aiChatBaseUrl,
    aiChatModel,
    aiApiKey,
  );
  const aiConfigFingerprint = modelProviderConfigFingerprint(aiConfig);

  const refreshRecentObjects = useCallback(() => {
    return loadRecentObjects({ limit: 50, offset: 0 });
  }, [loadRecentObjects]);

  const refreshSearchResults = useCallback(() => {
    const query = searchQuery.trim();

    if (!query) {
      resetSearch();
      return Promise.resolve([]);
    }

    return searchHybrid({ query, limit: 25 });
  }, [resetSearch, searchHybrid, searchQuery]);

  useEffect(() => {
    void refreshRecentObjects();
  }, [refreshRecentObjects]);

  useEffect(() => {
    void getModelProviderConfig();
  }, [getModelProviderConfig]);

  useEffect(() => {
    if (!storedModelConfig || aiConfigDirtyRef.current) {
      return;
    }

    setAIProvider(storedModelConfig.provider);
    setAIApiFamily(storedModelConfig.apiFamily);
    setAIChatBaseUrl(storedModelConfig.chatBaseUrl ?? "");
    setAIChatModel(storedModelConfig.defaultChatModel ?? "");
    setAIHasApiKey(storedModelConfig.hasApiKey);
  }, [storedModelConfig]);

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

  const handleSaveAIConfig = useCallback(async () => {
    const config = createModelProviderConfig(
      aiProvider,
      aiApiFamily,
      aiChatBaseUrl,
      aiChatModel,
      aiApiKey,
    );
    const saved = await updateModelProviderConfig(config);

    if (saved) {
      aiConfigDirtyRef.current = false;
      setAIApiKey("");
      await getModelProviderConfig();
    }
  }, [
    aiApiFamily,
    aiApiKey,
    aiChatBaseUrl,
    aiChatModel,
    aiProvider,
    getModelProviderConfig,
    updateModelProviderConfig,
  ]);

  const handleTestAIConfig = useCallback(async () => {
    const config = createModelProviderConfig(
      aiProvider,
      aiApiFamily,
      aiChatBaseUrl,
      aiChatModel,
      aiApiKey,
    );
    const result = await testModelProviderConfig(config);
    if (result) {
      setTestedAIConfigFingerprint(modelProviderConfigFingerprint(config));
    }
  }, [aiApiFamily, aiApiKey, aiChatBaseUrl, aiChatModel, aiProvider, testModelProviderConfig]);

  const handleAIProviderChange = useCallback((value: string) => {
    aiConfigDirtyRef.current = true;
    setTestedAIConfigFingerprint(undefined);
    setAIProvider(value);
    setAIHasApiKey(false);

    const preset = MODEL_PROVIDER_PRESETS[value.trim().toLowerCase()];
    if (preset) {
      setAIApiFamily(preset.apiFamily);
      setAIChatBaseUrl(preset.chatBaseUrl);
      setAIChatModel(preset.chatModel);
    }
  }, []);

  const handleAIApiFamilyChange = useCallback((value: ModelApiFamily) => {
    aiConfigDirtyRef.current = true;
    setTestedAIConfigFingerprint(undefined);
    setAIApiFamily(value);
  }, []);

  const handleAIChatBaseUrlChange = useCallback((value: string) => {
    aiConfigDirtyRef.current = true;
    setTestedAIConfigFingerprint(undefined);
    setAIChatBaseUrl(value);
  }, []);

  const handleAIChatModelChange = useCallback((value: string) => {
    aiConfigDirtyRef.current = true;
    setTestedAIConfigFingerprint(undefined);
    setAIChatModel(value);
  }, []);

  const handleAIApiKeyChange = useCallback((value: string) => {
    aiConfigDirtyRef.current = true;
    setTestedAIConfigFingerprint(undefined);
    setAIApiKey(value);
  }, []);

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

  const handleRebuildSearchIndex = useCallback(async () => {
    const response = await rebuildSearchIndex();
    if (!response) {
      return;
    }

    await Promise.all([refreshRecentObjects(), refreshSearchResults()]);
  }, [rebuildSearchIndex, refreshRecentObjects, refreshSearchResults]);

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

  return (
    <AppShell>
      <ThreePaneLayout
        sidebar={<Sidebar />}
        list={
          <ObjectList
            objects={objects}
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
            searchMaintenanceLoading={rebuildSearchIndexLoading}
            searchMaintenanceError={rebuildSearchIndexError}
            searchMaintenanceMessage={
              rebuildSearchIndexResult ? `Indexed ${rebuildSearchIndexResult.indexedObjects} objects` : undefined
            }
            onCaptureValueChange={setCaptureUrl}
            onCaptureSubmit={handleCaptureSubmit}
            onSearchValueChange={setSearchQuery}
            onClearSearch={() => {
              setSearchQuery("");
              resetSearch();
            }}
            onRebuildSearchIndex={handleRebuildSearchIndex}
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
            aiProvider={aiProvider}
            aiApiFamily={aiApiFamily}
            aiChatBaseUrl={aiChatBaseUrl}
            aiChatModel={aiChatModel}
            aiApiKey={aiApiKey}
            aiHasApiKey={aiHasApiKey}
            aiConfigLoading={getModelConfigLoading || updateModelConfigLoading}
            aiTestLoading={testModelConfigLoading}
            aiRunLoading={triggerAILoading}
            aiTestResult={
              testedAIConfigFingerprint === aiConfigFingerprint ? testModelConfigResult : undefined
            }
            aiError={
              triggerAIError ??
              testModelConfigError ??
              updateModelConfigError ??
              getModelConfigError ??
              aiRunFailureToError(aiRunResult)
            }
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
            onAIProviderChange={handleAIProviderChange}
            onAIApiFamilyChange={handleAIApiFamilyChange}
            onAIChatBaseUrlChange={handleAIChatBaseUrlChange}
            onAIChatModelChange={handleAIChatModelChange}
            onAIApiKeyChange={handleAIApiKeyChange}
            onSaveAIConfig={handleSaveAIConfig}
            onTestAIConfig={handleTestAIConfig}
            onRunAIAnalysis={handleRunAIAnalysis}
            onReindexObject={handleReindexSelectedObject}
            onRunEvaluation={handleRunEvaluation}
          />
        }
      />
    </AppShell>
  );
}

function createModelProviderConfig(
  provider: string,
  apiFamily: ModelApiFamily,
  chatBaseUrl: string,
  chatModel: string,
  apiKey: string,
): ModelProviderConfig {
  return {
    provider: provider.trim(),
    apiFamily,
    chatBaseUrl: chatBaseUrl.trim(),
    apiKey: apiKey.trim() || undefined,
    defaultChatModel: chatModel.trim(),
    capabilities: ["chat"],
  };
}

function modelProviderConfigFingerprint(config: ModelProviderConfig) {
  return JSON.stringify({
    provider: config.provider,
    apiFamily: config.apiFamily,
    chatBaseUrl: config.chatBaseUrl,
    defaultChatModel: config.defaultChatModel,
    capabilities: config.capabilities,
  });
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

  return {
    code: "ERR_MODEL_OUTPUT_SCHEMA",
    title: "AI analysis failed",
    message: run.failureReason ?? "The model provider did not return a usable analysis.",
    retryable: true,
    action: "retry" as const,
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

