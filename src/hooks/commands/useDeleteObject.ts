import { useCallback, useState } from "react";
import { invokeCommand } from "../../lib/tauri";
import type { AppUiError } from "../../lib/errors";
import type { DeleteObjectMode, DeleteObjectResponse } from "../../types/api";

interface DeleteObjectState {
  data?: DeleteObjectResponse;
  error?: AppUiError;
  loading: boolean;
}

interface DeleteObjectArgs {
  objectId: string;
  mode: DeleteObjectMode;
}

export function useDeleteObject() {
  const [state, setState] = useState<DeleteObjectState>({ loading: false });

  const deleteObject = useCallback(async (args: DeleteObjectArgs) => {
    setState({ loading: true });

    try {
      const data = await invokeCommand<DeleteObjectArgs, DeleteObjectResponse>("delete_object", args);
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  return { ...state, deleteObject };
}
