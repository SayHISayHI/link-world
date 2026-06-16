import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { RawCaptureItem, SubmitCaptureResponse } from "../../types/api";

interface SubmitCaptureState {
  data?: SubmitCaptureResponse;
  error?: AppUiError;
  loading: boolean;
}

export function useSubmitCapture() {
  const [state, setState] = useState<SubmitCaptureState>({ loading: false });

  const submitCapture = useCallback(async (item: RawCaptureItem) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<{ item: RawCaptureItem }, SubmitCaptureResponse>("submit_capture", {
        item,
      });
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, submitCapture };
}
