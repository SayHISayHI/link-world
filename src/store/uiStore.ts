import { create } from "zustand";
import { persist } from "zustand/middleware";
import { initialRoute, type AppRoute } from "../app/routes";

// Kept stable during the Node Tide rename so existing UI preferences survive the upgrade.
const LEGACY_UI_STORAGE_KEY = "link-world-ui";

export type AppLocale = "en" | "zh-CN";
export type AppTheme = "light" | "dark";

export function systemLocale(): AppLocale {
  return typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")
    ? "zh-CN"
    : "en";
}

export function systemTheme(): AppTheme {
  return typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

interface UiStore {
  route: AppRoute;
  locale: AppLocale;
  theme: AppTheme;
  sidebarCollapsed: boolean;
  detailPaneCollapsed: boolean;
  detailSidebarCollapsed: boolean;
  paneWidths: {
    sidebar: number;
    list: number;
    detailSidebar: number;
  };
  setRoute: (route: AppRoute) => void;
  setLocale: (locale: AppLocale) => void;
  setTheme: (theme: AppTheme) => void;
  toggleLocale: () => void;
  toggleTheme: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setDetailPaneCollapsed: (collapsed: boolean) => void;
  setDetailSidebarCollapsed: (collapsed: boolean) => void;
  setPaneWidth: (pane: keyof UiStore["paneWidths"], width: number) => void;
}

export const useUiStore = create<UiStore>()(
  persist(
    (set) => ({
      route: initialRoute,
      locale: systemLocale(),
      theme: systemTheme(),
      sidebarCollapsed: false,
      detailPaneCollapsed: false,
      detailSidebarCollapsed: false,
      paneWidths: {
        sidebar: 232,
        list: 360,
        detailSidebar: 320,
      },
      setRoute: (route) => set({ route }),
      setLocale: (locale) => set({ locale }),
      setTheme: (theme) => set({ theme }),
      toggleLocale: () =>
        set((state) => ({ locale: state.locale === "zh-CN" ? "en" : "zh-CN" })),
      toggleTheme: () =>
        set((state) => ({ theme: state.theme === "dark" ? "light" : "dark" })),
      setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
      setDetailPaneCollapsed: (detailPaneCollapsed) => set({ detailPaneCollapsed }),
      setDetailSidebarCollapsed: (detailSidebarCollapsed) => set({ detailSidebarCollapsed }),
      setPaneWidth: (pane, width) =>
        set((state) => ({
          paneWidths: { ...state.paneWidths, [pane]: width },
        })),
    }),
    {
      name: LEGACY_UI_STORAGE_KEY,
      partialize: (state) => ({ 
        locale: state.locale,
        theme: state.theme,
        paneWidths: state.paneWidths, 
        sidebarCollapsed: state.sidebarCollapsed,
        detailPaneCollapsed: state.detailPaneCollapsed,
        detailSidebarCollapsed: state.detailSidebarCollapsed
      }),
    }
  )
);

