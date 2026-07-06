import { useCallback, useRef, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObject } from "../../types/api";

interface RecentObjectsState {
  data: KnowledgeObject[];
  error?: AppUiError;
  loading: boolean;
}

interface LoadRecentObjectsArgs {
  limit?: number;
  offset?: number;
  filterType?: string;
  append?: boolean;
}

export function useRecentObjects() {
  const latestRequestIdRef = useRef(0);
  const [state, setState] = useState<RecentObjectsState>({
    data: [],
    loading: false,
  });

  const loadRecentObjects = useCallback(async (args: LoadRecentObjectsArgs = {}) => {
    const requestId = ++latestRequestIdRef.current;
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const page = await invokeCommand<
        Omit<LoadRecentObjectsArgs, "append">,
        KnowledgeObject[]
      >("get_recent_objects", {
        limit: args.limit ?? 50,
        offset: args.offset ?? 0,
        filterType: args.filterType,
      });

      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState((current) => ({
        data: args.append ? mergeObjects(current.data, page) : page,
        loading: false,
      }));
      return page;
    } catch (error) {
      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState((current) => ({
        data: args.append ? current.data : [],
        error: error as AppUiError,
        loading: false,
      }));
      return [];
    }
  }, []);

  return { ...state, loadRecentObjects };
}

function mergeObjects(current: KnowledgeObject[], page: KnowledgeObject[]) {
  const byId = new Map(current.map((object) => [object.id, object]));
  for (const object of page) {
    byId.set(object.id, object);
  }
  return Array.from(byId.values());
}
