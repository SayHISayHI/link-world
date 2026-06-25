import { useCallback, useState } from "react";
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
  const [state, setState] = useState<SearchHybridState>({ data: [], loading: false });

  const searchHybrid = useCallback(async (args: SearchHybridArgs) => {
    const query = args.query.trim();
    if (!query) {
      setState({ data: [], loading: false });
      return [];
    }

    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<SearchHybridArgs, SearchResult[]>("search_hybrid", {
        query,
        limit: args.limit,
        filterType: args.filterType,
      });
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ data: [], error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetSearch = useCallback(() => {
    setState({ data: [], loading: false });
  }, []);

  return { ...state, resetSearch, searchHybrid };
}
