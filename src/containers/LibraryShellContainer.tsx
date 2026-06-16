import { useEffect } from "react";
import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { useObjectDetail } from "../hooks/commands/useObjectDetail";
import { usePing } from "../hooks/commands/usePing";
import { useRecentObjects } from "../hooks/commands/useRecentObjects";
import { useLibraryStore } from "../store/libraryStore";

export function LibraryShellContainer() {
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

  useEffect(() => {
    void loadRecentObjects({ limit: 50, offset: 0 });
  }, [loadRecentObjects]);

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

