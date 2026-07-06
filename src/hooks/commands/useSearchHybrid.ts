import { useCallback, useRef, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { SearchResult } from "../../types/api";

interface SearchHybridState {
  data: SearchResult[];
  error?: AppUiError;
  loading: boolean;
}

interface SearchHybridArgs {
  query: string;
  limit?: number;
  filterType?: string;
}

export function useSearchHybrid() {
  const latestRequestIdRef = useRef(0);
  const [state, setState] = useState<SearchHybridState>({ data: [], loading: false });

  const searchHybrid = useCallback(async (args: SearchHybridArgs) => {
    const query = args.query.trim();
    if (!query) {
      latestRequestIdRef.current += 1;
      setState({ data: [], loading: false });
      return [];
    }

    const requestId = ++latestRequestIdRef.current;
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<SearchHybridArgs, SearchResult[]>("search_hybrid", {
        query,
        limit: args.limit,
        filterType: args.filterType,
      });

      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState({ data, loading: false });
      return data;
    } catch (error) {
      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState({ data: [], error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetSearch = useCallback(() => {
    latestRequestIdRef.current += 1;
    setState({ data: [], loading: false });
  }, []);

  return { ...state, resetSearch, searchHybrid };
}
