import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { PortableExportSummary } from "../../types/api";

interface PortableExportState {
  error?: AppUiError;
  exporting: boolean;
  summary?: PortableExportSummary;
}

const initialState: PortableExportState = {
  exporting: false,
};

export function usePortableExport() {
  const [state, setState] = useState<PortableExportState>(initialState);

  const exportLibrary = useCallback(async () => {
    setState((current) => ({
      ...current,
      error: undefined,
      exporting: true,
    }));
    try {
      const summary = await invokeCommand<Record<string, never>, PortableExportSummary>(
        "export_library",
        {},
      );
      setState({ exporting: false, summary });
      return summary;
    } catch (error) {
      setState((current) => ({
        ...current,
        error: error as AppUiError,
        exporting: false,
      }));
      return undefined;
    }
  }, []);

  return {
    ...state,
    exportLibrary,
  };
}
