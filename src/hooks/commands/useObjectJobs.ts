import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { BackgroundJob } from "../../types/api";

interface ObjectJobsState {
  data: BackgroundJob[];
  error?: AppUiError;
  loading: boolean;
}

interface LoadObjectJobsArgs {
  objectId: string;
  limit?: number;
}

export function useObjectJobs() {
  const [state, setState] = useState<ObjectJobsState>({
    data: [],
    loading: false,
  });

  const loadObjectJobs = useCallback(async (args: LoadObjectJobsArgs) => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<LoadObjectJobsArgs, BackgroundJob[]>("get_object_jobs", {
        objectId: args.objectId,
        limit: args.limit ?? 10,
      });
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ data: [], error: error as AppUiError, loading: false });
      return [];
    }
  }, []);

  const resetObjectJobs = useCallback(() => {
    setState({ data: [], loading: false });
  }, []);

  return { ...state, loadObjectJobs, resetObjectJobs };
}
