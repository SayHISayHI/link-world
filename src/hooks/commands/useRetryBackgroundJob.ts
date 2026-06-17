import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";

interface RetryBackgroundJobState {
  data?: boolean;
  error?: AppUiError;
  loading: boolean;
}

interface RetryBackgroundJobArgs {
  jobId: string;
}

export function useRetryBackgroundJob() {
  const [state, setState] = useState<RetryBackgroundJobState>({ loading: false });

  const retryBackgroundJob = useCallback(async (args: RetryBackgroundJobArgs) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<RetryBackgroundJobArgs, boolean>("retry_background_job", args);
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return false;
    }
  }, []);

  return { ...state, retryBackgroundJob };
}
