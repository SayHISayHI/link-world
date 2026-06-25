import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";

interface RestartAppState {
  error?: AppUiError;
  loading: boolean;
}

export function useRestartApp() {
  const [state, setState] = useState<RestartAppState>({ loading: false });

  const restartApp = useCallback(async () => {
    setState({ loading: true });
    try {
      const scheduled = await invokeCommand<Record<string, never>, boolean>(
        "restart_app",
        {},
      );
      setState({ loading: false });
      return scheduled;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return false;
    }
  }, []);

  return { ...state, restartApp };
}
