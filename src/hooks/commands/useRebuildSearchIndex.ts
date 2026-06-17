import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { RebuildSearchIndexResponse } from "../../types/api";

interface RebuildSearchIndexState {
  data?: RebuildSearchIndexResponse;
  error?: AppUiError;
  loading: boolean;
}

export function useRebuildSearchIndex() {
  const [state, setState] = useState<RebuildSearchIndexState>({ loading: false });

  const rebuildSearchIndex = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<Record<string, never>, RebuildSearchIndexResponse>(
        "rebuild_search_index",
        {},
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, rebuildSearchIndex };
}
