import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { StartupStatus } from "../../types/api";

interface StartupStatusState {
  error?: AppUiError;
  loading: boolean;
  status?: StartupStatus;
}

export function useStartupStatus() {
  const [state, setState] = useState<StartupStatusState>({ loading: true });

  const loadStartupStatus = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));
    try {
      const status = await invokeCommand<Record<string, never>, StartupStatus>(
        "get_startup_status",
        {},
      );
      setState({ loading: false, status });
      return status;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, loadStartupStatus };
}
