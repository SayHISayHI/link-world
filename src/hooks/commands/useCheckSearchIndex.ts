import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { SearchIndexHealthResponse } from "../../types/api";

interface CheckSearchIndexState {
  data?: SearchIndexHealthResponse;
  error?: AppUiError;
  loading: boolean;
}

export function useCheckSearchIndex() {
  const [state, setState] = useState<CheckSearchIndexState>({ loading: false });

  const checkSearchIndex = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<Record<string, never>, SearchIndexHealthResponse>(
        "check_search_index",
        {},
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, checkSearchIndex };
}
