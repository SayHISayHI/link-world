import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { PingResponse } from "../../types/api";

interface PingState {
  data?: PingResponse;
  error?: AppUiError;
  loading: boolean;
}

export function usePing() {
  const [state, setState] = useState<PingState>({ loading: false });

  const ping = useCallback(async () => {
    setState({ loading: true });
    try {
      const data = await invokeCommand<Record<string, never>, PingResponse>("ping", {});
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, ping };
}

