import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { LibraryNavigation } from "../../types/api";

interface NavigationState {
  data?: LibraryNavigation;
  error?: AppUiError;
  loading: boolean;
}

export function useLibraryNavigation() {
  const [state, setState] = useState<NavigationState>({ loading: false });

  const loadNavigation = useCallback(async () => {
    setState((current) => ({ ...current, error: undefined, loading: true }));
    try {
      const data = await invokeCommand<Record<string, never>, LibraryNavigation>(
        "get_library_navigation",
        {},
      );
      setState({ data, loading: false });
      return data;
    } catch (error) {
      setState((current) => ({ ...current, error: error as AppUiError, loading: false }));
      return undefined;
    }
  }, []);

  return { ...state, loadNavigation };
}
