import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { ModelProviderConfigView } from "../../types/api";

interface GetModelProviderConfigState {
  data?: ModelProviderConfigView | null;
  error?: AppUiError;
  loading: boolean;
}

export function useGetModelProviderConfig() {
  const [state, setState] = useState<GetModelProviderConfigState>({ loading: false });

  const getModelProviderConfig = useCallback(async () => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<Record<string, never>, ModelProviderConfigView | null>(
        "get_model_provider_config",
        {},
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, getModelProviderConfig };
}
