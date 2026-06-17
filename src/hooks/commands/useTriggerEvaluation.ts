import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { TriggerEvaluationResponse } from "../../types/api";

interface TriggerEvaluationState {
  data?: TriggerEvaluationResponse;
  error?: AppUiError;
  loading: boolean;
}

interface TriggerEvaluationArgs {
  objectId: string;
  evaluatorType: string;
}

export function useTriggerEvaluation() {
  const [state, setState] = useState<TriggerEvaluationState>({ loading: false });

  const triggerEvaluation = useCallback(async (args: TriggerEvaluationArgs) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<TriggerEvaluationArgs, TriggerEvaluationResponse>(
        "trigger_evaluation",
        args,
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, triggerEvaluation };
}
