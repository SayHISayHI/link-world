import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { AIEnrichmentRunResult } from "../../types/api";

interface TriggerAIEnrichmentState {
  data?: AIEnrichmentRunResult;
  error?: AppUiError;
  loading: boolean;
}

interface TriggerAIEnrichmentArgs {
  objectId: string;
}

export function useTriggerAIEnrichment() {
  const [state, setState] = useState<TriggerAIEnrichmentState>({ loading: false });

  const triggerAIEnrichment = useCallback(async (args: TriggerAIEnrichmentArgs) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<TriggerAIEnrichmentArgs, AIEnrichmentRunResult>(
        "trigger_ai_enrichment",
        args,
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, triggerAIEnrichment };
}
