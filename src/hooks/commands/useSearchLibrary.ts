import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { LibraryQuery, SearchResult } from "../../types/api";

interface SearchState {
  data: SearchResult[];
  error?: AppUiError;
  loading: boolean;
}

export function useSearchLibrary() {
  const [state, setState] = useState<SearchState>({ data: [], loading: false });

  const searchLibrary = useCallback(
    async ({ query, limit, libraryQuery }: { query: string; limit?: number; libraryQuery: LibraryQuery }) => {
      const normalized = query.trim();
      if (!normalized) {
        setState({ data: [], loading: false });
        return [];
      }
      setState((current) => ({ ...current, error: undefined, loading: true }));
      try {
        const data = await invokeCommand<
          { query: string; limit?: number; libraryQuery: LibraryQuery },
          SearchResult[]
        >("search_library", { query: normalized, limit, libraryQuery });
        setState({ data, loading: false });
        return data;
      } catch (error) {
        setState({ data: [], error: error as AppUiError, loading: false });
        return undefined;
      }
    },
    [],
  );

  const resetSearch = useCallback(() => setState({ data: [], loading: false }), []);
  return { ...state, searchLibrary, resetSearch };
}
