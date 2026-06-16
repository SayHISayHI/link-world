import { useCallback, useEffect, useState } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { usePing } from "../hooks/commands/usePing";
import { useRecentObjects } from "../hooks/commands/useRecentObjects";
import { useSubmitCapture } from "../hooks/commands/useSubmitCapture";
import { useLibraryStore } from "../store/libraryStore";

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
  }, [loadObjectDetail, refreshRecentObjects, selectedObjectId]);

  useEffect(() => {
    setObjects(recentObjects);
  }, [recentObjects, setObjects]);

  useEffect(() => {
    if (!selectedObjectId) {
      resetObjectDetail();
      setSelectedDetail(undefined);
      return;
    }

    void loadObjectDetail(selectedObjectId);
  }, [loadObjectDetail, resetObjectDetail, selectedObjectId, setSelectedDetail]);

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
            onPing={() => {
              void ping();
            }}
          />
        }
      />
    </AppShell>
  );
}

