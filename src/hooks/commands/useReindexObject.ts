import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { ReindexObjectResponse } from "../../types/api";

interface ReindexObjectState {
  data?: ReindexObjectResponse;
  error?: AppUiError;
  loading: boolean;
}

interface ReindexObjectArgs {
  objectId: string;
}

export function useReindexObject() {
  const [state, setState] = useState<ReindexObjectState>({ loading: false });

  const reindexObject = useCallback(async (args: ReindexObjectArgs) => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<ReindexObjectArgs, ReindexObjectResponse>("reindex_object", args);
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, reindexObject };
}
