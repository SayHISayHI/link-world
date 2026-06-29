import { useCallback, useState } from "react";
import type { AppUiError } from "../../lib/errors";
import { invokeCommand } from "../../lib/tauri";
import type { SupportBundleSummary } from "../../types/api";

interface SupportBundleExportState {
  error?: AppUiError;
  exporting: boolean;
  summary?: SupportBundleSummary;
}

const initialState: SupportBundleExportState = {
  exporting: false,
};

export function useSupportBundleExport() {
  const [state, setState] = useState<SupportBundleExportState>(initialState);

  const exportSupportBundle = useCallback(async ({ confirmed }: { confirmed: boolean }) => {
    setState((current) => ({
      ...current,
      error: undefined,
      exporting: true,
    }));

    try {
      const summary = await invokeCommand<{ confirmed: boolean }, SupportBundleSummary>(
        "export_support_bundle",
        { confirmed },
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
    exportSupportBundle,
  };
}