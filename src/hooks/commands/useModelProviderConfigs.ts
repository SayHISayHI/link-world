import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type {
  ModelProviderConfig,
  ModelProviderConfigView,
  ModelProviderTestResult,
} from "../../types/api";

interface ModelProviderConfigsState {
  configs: ModelProviderConfigView[];
  error?: AppUiError;
  loading: boolean;
  mutating: boolean;
  testError?: AppUiError;
  testLoading: boolean;
  testResult?: ModelProviderTestResult;
}

const initialState: ModelProviderConfigsState = {
  configs: [],
  loading: false,
  mutating: false,
  testLoading: false,
};

export function useModelProviderConfigs() {
  const [state, setState] = useState<ModelProviderConfigsState>(initialState);

  const loadConfigs = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));
    try {
      const configs = await invokeCommand<Record<string, never>, ModelProviderConfigView[]>(
        "list_model_provider_configs",
        {},
      );
      setState((current) => ({ ...current, configs, loading: false }));
      return configs;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, loading: false }));
      return undefined;
    }
  }, []);

  const saveConfig = useCallback(async (config: ModelProviderConfig) => {
    setState((current) => ({ ...current, error: undefined, mutating: true }));
    try {
      const saved = await invokeCommand<{ config: ModelProviderConfig }, ModelProviderConfigView>(
        "save_model_provider_config",
        { config },
      );
      setState((current) => {
        const configs = [saved, ...current.configs.filter((item) => item.id !== saved.id)];
        return { ...current, configs, mutating: false };
      });
      return saved;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, mutating: false }));
      return undefined;
    }
  }, []);

  const deleteConfig = useCallback(async (configId: string) => {
    setState((current) => ({ ...current, error: undefined, mutating: true }));
    try {
      await invokeCommand<{ configId: string }, boolean>("delete_model_provider_config", {
        configId,
      });
      setState((current) => ({
        ...current,
        configs: current.configs.filter((item) => item.id !== configId),
        mutating: false,
      }));
      return true;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, mutating: false }));
      return false;
    }
  }, []);

  const setDefault = useCallback(async (configId: string) => {
    setState((current) => ({ ...current, error: undefined, mutating: true }));
    try {
      await invokeCommand<{ configId: string }, boolean>("set_default_model_provider", {
        configId,
      });
      setState((current) => ({
        ...current,
        configs: current.configs.map((item) => ({
          ...item,
          isDefault: item.id === configId,
        })),
        mutating: false,
      }));
      return true;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, mutating: false }));
      return false;
    }
  }, []);

  const testConfig = useCallback(async (config: ModelProviderConfig) => {
    setState((current) => ({
      ...current,
      testError: undefined,
      testLoading: true,
      testResult: undefined,
    }));
    try {
      const testResult = await invokeCommand<
        { config: ModelProviderConfig },
        ModelProviderTestResult
      >("test_model_provider_config", { config });
      setState((current) => ({ ...current, testLoading: false, testResult }));
      return testResult;
    } catch (error) {
      setState((current) => ({
        ...current,
        testError: error as AppUiError,
        testLoading: false,
      }));
      return undefined;
    }
  }, []);

  const clearTestResult = useCallback(() => {
    setState((current) => ({
      ...current,
      testError: undefined,
      testResult: undefined,
    }));
  }, []);

  return {
    ...state,
    clearTestResult,
    deleteConfig,
    loadConfigs,
    saveConfig,
    setDefault,
    testConfig,
  };
}
