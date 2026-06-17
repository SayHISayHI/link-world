import { useCallback, useEffect, useState } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { useDeleteObject } from "../hooks/commands/useDeleteObject";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { useObjectJobs } from "../hooks/commands/useObjectJobs";
import { usePing } from "../hooks/commands/usePing";
import { useRecentObjects } from "../hooks/commands/useRecentObjects";
import { useRetryBackgroundJob } from "../hooks/commands/useRetryBackgroundJob";
import { useSubmitCapture } from "../hooks/commands/useSubmitCapture";
import { useLibraryStore } from "../store/libraryStore";
import type { BackgroundJob } from "../types/api";

interface CaptureJobCompletedPayload {
  jobId: string;
  status: "succeeded" | "failed" | "skipped" | string;
  objectId?: string;
  lifecycleStatus?: string;
  parsedDocumentId?: string;
  failureReason?: string;
}

export function LibraryShellContainer() {
  const [captureUrl, setCaptureUrl] = useState("");
  const [lastCaptureJob, setLastCaptureJob] = useState<CaptureJobCompletedPayload>();
  const { objects, selectedObjectId, selectedDetail, selectObject, setObjects, setSelectedDetail } = useLibraryStore();
  const selectedObject = objects.find((object) => object.id === selectedObjectId);
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
  const retryableCaptureJob = findRetryableCaptureJob(objectJobs, selectedObject?.id);

  const refreshRecentObjects = useCallback(() => {
    return loadRecentObjects({ limit: 50, offset: 0 });
  }, [loadRecentObjects]);

  useEffect(() => {
    void refreshRecentObjects();
  }, [refreshRecentObjects]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unlistenCapture: (() => void) | undefined;
    let disposed = false;

    void import("@tauri-apps/api/event")
      .then(async ({ listen }) => {
        const unsubscribeLibrary = await listen("library://objects-updated", () => {
          void refreshRecentObjects();
        });
        const unsubscribeCapture = await listen<CaptureJobCompletedPayload>("capture://job-completed", (event) => {
          setLastCaptureJob(event.payload);
          void refreshRecentObjects();

          const objectId = event.payload.objectId;
          if (objectId && objectId === selectedObjectId) {
            void loadObjectDetail(objectId);
            void loadObjectJobs({ objectId, limit: 10 });
          }
        });

        return { unsubscribeCapture, unsubscribeLibrary };
      })
      .then(({ unsubscribeCapture, unsubscribeLibrary }) => {
        if (disposed) {
          unsubscribeLibrary();
          unsubscribeCapture();
          return;
        }

        unlisten = unsubscribeLibrary;
        unlistenCapture = unsubscribeCapture;
      })
      .catch(() => {
        unlisten = undefined;
        unlistenCapture = undefined;
      });

    return () => {
      disposed = true;
      unlisten?.();
      unlistenCapture?.();
    };
  }, [loadObjectDetail, loadObjectJobs, refreshRecentObjects, selectedObjectId]);

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
    setObjects(nextObjects);
  }, [
    deleteObject,
    refreshRecentObjects,
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
    if (selectedObjectId) {
      await loadObjectDetail(selectedObjectId);
      await loadObjectJobs({ objectId: selectedObjectId, limit: 10 });
    }
  }, [
    loadObjectDetail,
    loadObjectJobs,
    refreshRecentObjects,
    retryBackgroundJob,
    retryableCaptureJob,
    selectedObjectId,
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
            onCaptureValueChange={setCaptureUrl}
            onCaptureSubmit={handleCaptureSubmit}
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
            onPing={() => {
              void ping();
            }}
            onDeleteObject={handleDeleteObject}
            onRetryCapture={handleRetryCapture}
          />
        }
      />
    </AppShell>
  );
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

