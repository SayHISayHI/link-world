import { useCallback, useEffect, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type {
  SourceCredentialListResponse,
  SourcePlatformCredential,
  SourceProbeResponse,
} from "../../types/api";

interface SourceCredentialState {
  data?: SourceCredentialListResponse;
  error?: AppUiError;
  loading: boolean;
  saving: boolean;
  probing: boolean;
  probe?: SourceProbeResponse;
  probeError?: AppUiError;
}

export function useSourceCredentials() {
  const [state, setState] = useState<SourceCredentialState>({
    loading: false,
    saving: false,
    probing: false,
  });

  const load = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<Record<string, never>, SourceCredentialListResponse>(
        "list_source_credentials",
        {},
      );
      setState((current) => ({ ...current, data, loading: false }));
      return data;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, loading: false }));
      return undefined;
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const saveCredential = useCallback(
    async (domain: string, cookie: string) => {
      setState((current) => ({ ...current, error: undefined, saving: true }));

      try {
        const saved = await invokeCommand<
          { domain: string; cookie: string },
          SourcePlatformCredential
        >("save_source_credential", { domain, cookie });
        await load();
        setState((current) => ({ ...current, saving: false }));
        return saved;
      } catch (error) {
        setState((current) => ({ ...current, error: error as AppUiError, saving: false }));
        return undefined;
      }
    },
    [load],
  );

  const deleteCredential = useCallback(
    async (domain: string) => {
      setState((current) => ({ ...current, error: undefined, saving: true }));

      try {
        await invokeCommand<{ domain: string }, boolean>("delete_source_credential", { domain });
        await load();
        setState((current) => ({ ...current, saving: false }));
        return true;
      } catch (error) {
        setState((current) => ({ ...current, error: error as AppUiError, saving: false }));
        return false;
      }
    },
    [load],
  );

  const probeUrl = useCallback(async (url: string) => {
    setState((current) => ({
      ...current,
      probe: undefined,
      probeError: undefined,
      probing: true,
    }));

    try {
      const probe = await invokeCommand<{ url: string }, SourceProbeResponse>(
        "probe_source_url",
        { url },
      );
      setState((current) => ({ ...current, probe, probing: false }));
      return probe;
    } catch (error) {
      setState((current) => ({
        ...current,
        probeError: error as AppUiError,
        probing: false,
      }));
      return undefined;
    }
  }, []);

  return { ...state, load, saveCredential, deleteCredential, probeUrl };
}
