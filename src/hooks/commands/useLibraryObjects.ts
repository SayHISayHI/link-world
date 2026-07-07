import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { KnowledgeObject, LibraryPage, LibraryQuery } from "../../types/api";

interface LibraryObjectsState {
  data: KnowledgeObject[];
  nextCursor?: string;
  error?: AppUiError;
  loading: boolean;
}

export function useLibraryObjects() {
  const [state, setState] = useState<LibraryObjectsState>({ data: [], loading: false });

  const loadLibraryObjects = useCallback(
    async ({ query, append = false }: { query: LibraryQuery; append?: boolean }) => {
      setState((current) => ({ ...current, error: undefined, loading: true }));
      try {
        const page = await invokeCommand<{ query: LibraryQuery }, LibraryPage<KnowledgeObject>>(
          "list_library_objects",
          { query },
        );
        setState((current) => ({
          data: append ? mergeObjects(current.data, page.items) : page.items,
          nextCursor: page.nextCursor,
          loading: false,
        }));
        return page;
      } catch (error) {
        setState((current) => ({
          data: append ? current.data : [],
          nextCursor: append ? current.nextCursor : undefined,
          error: error as AppUiError,
          loading: false,
        }));
        return undefined;
      }
    },
    [],
  );

  return { ...state, loadLibraryObjects };
}

function mergeObjects(current: KnowledgeObject[], next: KnowledgeObject[]) {
  const byId = new Map(current.map((object) => [object.id, object]));
  for (const object of next) {
    byId.set(object.id, object);
  }
  return Array.from(byId.values());
}
