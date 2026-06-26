import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { LocalMetricsSnapshot } from "../../types/api";

interface LocalMetricsSnapshotState {
  data?: LocalMetricsSnapshot;
  error?: AppUiError;
  loading: boolean;
}

export function useLocalMetricsSnapshot() {
  const [state, setState] = useState<LocalMetricsSnapshotState>({ loading: false });

  const loadSnapshot = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<Record<string, never>, LocalMetricsSnapshot>(
        "get_local_metrics_snapshot",
        {},
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, loadSnapshot };
}
