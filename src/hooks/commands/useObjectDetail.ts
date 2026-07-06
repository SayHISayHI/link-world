import { useCallback, useRef, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObjectDetail } from "../../types/api";

interface ObjectDetailState {
  data?: KnowledgeObjectDetail;
  error?: AppUiError;
  loading: boolean;
}

export function useObjectDetail() {
  const latestRequestIdRef = useRef(0);
  const [state, setState] = useState<ObjectDetailState>({ loading: false });

  const loadObjectDetail = useCallback(async (objectId: string) => {
    const requestId = ++latestRequestIdRef.current;
    setState({ loading: true });

    try {
      const data = await invokeCommand<{ objectId: string }, KnowledgeObjectDetail>("get_object_detail", {
        objectId,
      });

      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState({ data, loading: false });
      return data;
    } catch (error) {
      if (latestRequestIdRef.current !== requestId) {
        return undefined;
      }

      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetObjectDetail = useCallback(() => {
    latestRequestIdRef.current += 1;
    setState({ loading: false });
  }, []);

  return { ...state, loadObjectDetail, resetObjectDetail };
}
