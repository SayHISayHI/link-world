import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { TriggerEvaluationResponse } from "../../types/api";

interface RetryEvaluationState {
  data?: TriggerEvaluationResponse;
  error?: AppUiError;
  loading: boolean;
}

interface RetryEvaluationArgs {
  runId: string;
  requestId?: string;
}

export function useRetryEvaluation() {
  const [state, setState] = useState<RetryEvaluationState>({ loading: false });

  const retryEvaluation = useCallback(async (args: RetryEvaluationArgs) => {
    setState({ loading: true });

    try {
      const request = {
        ...args,
        requestId: args.requestId ?? globalThis.crypto.randomUUID(),
      };
      const data = await invokeCommand<RetryEvaluationArgs, TriggerEvaluationResponse>(
        "retry_evaluation",
        request,
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetRetryEvaluation = useCallback(() => {
    setState({ loading: false });
  }, []);

  return { ...state, retryEvaluation, resetRetryEvaluation };
}
