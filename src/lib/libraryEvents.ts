import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface CaptureJobCompletedPayload {
  jobId?: string;
  status: "succeeded" | "failed" | "skipped" | string;
  objectId?: string;
  lifecycleStatus?: string;
  parsedDocumentId?: string;
  failureReason?: string;
}

export interface AIEnrichmentCompletedPayload {
  jobId?: string;
  status: "succeeded" | "failed" | string;
  objectId?: string;
  analysisId?: string;
  failureReason?: string;
}

export interface EvaluationCompletedPayload {
  objectId?: string;
  runId?: string;
}

export interface SearchIndexUpdatedPayload {
  jobId?: string;
  objectId?: string;
  indexed?: boolean;
  indexedObjects?: number;
}

interface LibraryEventHandlers {
  onObjectsUpdated: () => void;
  onCaptureJobCompleted: (payload: CaptureJobCompletedPayload) => void;
  onAIEnrichmentCompleted: (payload: AIEnrichmentCompletedPayload) => void;
  onEvaluationCompleted: (payload: EvaluationCompletedPayload) => void;
  onSearchIndexRebuilt: (payload: SearchIndexUpdatedPayload) => void;
  onObjectReindexed: (payload: SearchIndexUpdatedPayload) => void;
}

export async function subscribeToLibraryEvents(
  handlers: LibraryEventHandlers,
): Promise<UnlistenFn> {
  const unlisteners: UnlistenFn[] = [];

  try {
    unlisteners.push(
      await listen("library://objects-updated", handlers.onObjectsUpdated),
    );
    unlisteners.push(
      await listen<CaptureJobCompletedPayload>("capture://job-completed", (event) => {
        handlers.onCaptureJobCompleted(event.payload);
      }),
    );
    unlisteners.push(
      await listen<AIEnrichmentCompletedPayload>("ai://enrichment-completed", (event) => {
        handlers.onAIEnrichmentCompleted(event.payload);
      }),
    );
    unlisteners.push(
      await listen<EvaluationCompletedPayload>("evaluation://completed", (event) => {
        handlers.onEvaluationCompleted(event.payload);
      }),
    );
    unlisteners.push(
      await listen<SearchIndexUpdatedPayload>("search://index-rebuilt", (event) => {
        handlers.onSearchIndexRebuilt(event.payload);
      }),
    );
    unlisteners.push(
      await listen<SearchIndexUpdatedPayload>("search://object-reindexed", (event) => {
        handlers.onObjectReindexed(event.payload);
      }),
    );
  } catch (error) {
    unlisteners.forEach((unlisten) => unlisten());
    throw error;
  }

  return () => {
    unlisteners.forEach((unlisten) => unlisten());
  };
}
