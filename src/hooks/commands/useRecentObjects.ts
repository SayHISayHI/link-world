import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObject, KnowledgeObjectType } from "../../types/api";

interface RecentObjectsState {
  data: KnowledgeObject[];
  error?: AppUiError;
  loading: boolean;
}

interface LoadRecentObjectsArgs {
  limit?: number;
  offset?: number;
  filterType?: KnowledgeObjectType;
}

export function useRecentObjects() {
  const [state, setState] = useState<RecentObjectsState>({
    data: [],
    loading: false,
  });

  const loadRecentObjects = useCallback(async (args: LoadRecentObjectsArgs = {}) => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<LoadRecentObjectsArgs, KnowledgeObject[]>("get_recent_objects", {
        limit: args.limit ?? 50,
        offset: args.offset ?? 0,
        filterType: args.filterType,
      });
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ data: [], error: error as AppUiError, loading: false });
      return [];
    }
  }, []);

  return { ...state, loadRecentObjects };
}
