import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { KnowledgeObjectDetail } from "../../types/api";

interface ObjectDetailState {
  data?: KnowledgeObjectDetail;
  error?: AppUiError;
  loading: boolean;
}

export function useObjectDetail() {
  const [state, setState] = useState<ObjectDetailState>({ loading: false });

  const loadObjectDetail = useCallback(async (objectId: string) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<{ objectId: string }, KnowledgeObjectDetail>("get_object_detail", {
        objectId,
      });
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetObjectDetail = useCallback(() => {
    setState({ loading: false });
  }, []);

  return { ...state, loadObjectDetail, resetObjectDetail };
}
