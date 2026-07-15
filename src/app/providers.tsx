import { useLayoutEffect, type ReactNode } from "react";
import { useUiStore } from "../store/uiStore";
import { applyUiPreferences } from "./uiPreferences";

interface AppProvidersProps {
  children: ReactNode;
}

export function AppProviders({ children }: AppProvidersProps) {
  const locale = useUiStore((state) => state.locale);
  const theme = useUiStore((state) => state.theme);

  useLayoutEffect(() => {
    applyUiPreferences({ locale, theme });
  }, [locale, theme]);

  return children;
}

