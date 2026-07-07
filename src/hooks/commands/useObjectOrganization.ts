import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { ObjectOrganization } from "../../types/api";

interface OrganizationState {
  data?: ObjectOrganization;
  error?: AppUiError;
  loading: boolean;
}

export function useObjectOrganization() {
  const [state, setState] = useState<OrganizationState>({ loading: false });

  const loadObjectOrganization = useCallback(async (objectId: string) => {
    setState((current) => ({ ...current, error: undefined, loading: true }));
    try {
      const data = await invokeCommand<{ objectId: string }, ObjectOrganization>(
        "get_object_organization",
        { objectId },
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState({ error: error as AppUiError, loading: false });
      return undefined;
    }
  }, []);

  const resetObjectOrganization = useCallback(() => {
    setState({ loading: false });
  }, []);

  return { ...state, loadObjectOrganization, resetObjectOrganization };
}
