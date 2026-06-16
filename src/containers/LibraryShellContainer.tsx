import { AppShell } from "../components/layout/AppShell";
import { ThreePaneLayout } from "../components/layout/ThreePaneLayout";
import { ObjectDetail } from "../components/library/ObjectDetail";
import { ObjectList } from "../components/library/ObjectList";
import { Sidebar } from "../components/library/Sidebar";
import { usePing } from "../hooks/commands/usePing";
import { useLibraryStore } from "../store/libraryStore";

export function LibraryShellContainer() {
  const { objects, selectedObjectId, selectObject } = useLibraryStore();
  const selectedObject = objects.find((object) => object.id === selectedObjectId);
  const { data, error, loading, ping } = usePing();

  return (
    <AppShell>
      <ThreePaneLayout
        sidebar={<Sidebar />}
        list={
          <ObjectList
            objects={objects}
            selectedObjectId={selectedObjectId}
            onSelectObject={selectObject}
          />
        }
        detail={
          <ObjectDetail
            object={selectedObject}
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

