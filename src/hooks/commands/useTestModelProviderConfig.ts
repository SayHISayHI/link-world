import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { ModelProviderConfig, ModelProviderTestResult } from "../../types/api";

interface TestModelProviderConfigState {
  data?: ModelProviderTestResult;
  error?: AppUiError;
  loading: boolean;
}

export function useTestModelProviderConfig() {
  const [state, setState] = useState<TestModelProviderConfigState>({ loading: false });

  const testModelProviderConfig = useCallback(async (config: ModelProviderConfig) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<{ config: ModelProviderConfig }, ModelProviderTestResult>(
        "test_model_provider_config",
        { config },
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, testModelProviderConfig };
}
