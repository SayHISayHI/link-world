import { useCallback, useEffect, useRef, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { RebuildSearchIndexResponse } from "../../types/api";

interface RebuildSearchIndexState {
  data?: RebuildSearchIndexResponse;
  error?: AppUiError;
  loading: boolean;
}

export function useRebuildSearchIndex() {
  const [state, setState] = useState<RebuildSearchIndexState>({ loading: false });
  const pollTimeoutRef = useRef<number>();

  const stopPolling = useCallback(() => {
    if (pollTimeoutRef.current !== undefined) {
      window.clearTimeout(pollTimeoutRef.current);
      pollTimeoutRef.current = undefined;
    }
  }, []);

  const pollRebuildStatus = useCallback(
    (jobId: string) => {
      stopPolling();

      const poll = async () => {
        try {
          const data = await invokeCommand<{ jobId: string }, RebuildSearchIndexResponse>(
            "get_search_index_rebuild_status",
            { jobId },
          );
          const active = isRebuildActive(data);
          setState({ data, loading: active });

          if (active) {
            pollTimeoutRef.current = window.setTimeout(poll, 500);
          }
        } catch (error) {
          setState((current) => ({
            ...current,
            error: error as AppUiError,
            loading: false,
          }));
        }
      };

      pollTimeoutRef.current = window.setTimeout(poll, 500);
    },
    [stopPolling],
  );

  const rebuildSearchIndex = useCallback(async () => {
    stopPolling();
    setState((current) => ({ ...current, error: undefined, loading: true }));

    try {
      const data = await invokeCommand<Record<string, never>, RebuildSearchIndexResponse>(
        "rebuild_search_index",
        {},
      );
      const active = isRebuildActive(data);
      setState({ data, loading: active });
      if (active) {
        pollRebuildStatus(data.jobId);
      }
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, [pollRebuildStatus, stopPolling]);

  const cancelSearchIndexRebuild = useCallback(
    async (jobId: string) => {
      setState((current) => ({ ...current, error: undefined, loading: true }));

      try {
        const data = await invokeCommand<{ jobId: string }, RebuildSearchIndexResponse>(
          "cancel_search_index_rebuild",
          { jobId },
        );
        const active = isRebuildActive(data);
        setState({ data, loading: active });
        if (active) {
          pollRebuildStatus(data.jobId);
        } else {
          stopPolling();
        }
        return data;
      } catch (error) {
        setState((current) => ({
          ...current,
          error: error as AppUiError,
          loading: false,
        }));
        return undefined;
      }
    },
    [pollRebuildStatus, stopPolling],
  );

  useEffect(() => stopPolling, [stopPolling]);

  return { ...state, cancelSearchIndexRebuild, rebuildSearchIndex };
}

function isRebuildActive(response: RebuildSearchIndexResponse) {
  return response.status === "queued" || response.status === "running";
}
