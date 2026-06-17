import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { ModelProviderConfig } from "../../types/api";

interface UpdateModelProviderConfigState {
  data?: boolean;
  error?: AppUiError;
  loading: boolean;
}

export function useUpdateModelProviderConfig() {
  const [state, setState] = useState<UpdateModelProviderConfigState>({ loading: false });

  const updateModelProviderConfig = useCallback(async (config: ModelProviderConfig) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<{ config: ModelProviderConfig }, boolean>(
        "update_model_provider_config",
        { config },
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return false;
    }
  }, []);

  return { ...state, updateModelProviderConfig };
}
