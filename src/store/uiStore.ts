import { create } from "zustand";
import { persist } from "zustand/middleware";
import { initialRoute, type AppRoute } from "../app/routes";

interface UiStore {
  route: AppRoute;
  sidebarCollapsed: boolean;
  detailPaneCollapsed: boolean;
  detailSidebarCollapsed: boolean;
  paneWidths: {
    sidebar: number;
    list: number;
    detailSidebar: number;
  };
  setRoute: (route: AppRoute) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setDetailPaneCollapsed: (collapsed: boolean) => void;
  setDetailSidebarCollapsed: (collapsed: boolean) => void;
  setPaneWidth: (pane: keyof UiStore["paneWidths"], width: number) => void;
}

export const useUiStore = create<UiStore>()(
  persist(
    (set) => ({
      route: initialRoute,
      sidebarCollapsed: false,
      detailPaneCollapsed: false,
      detailSidebarCollapsed: false,
      paneWidths: {
        sidebar: 232,
        list: 360,
        detailSidebar: 320,
      },
      setRoute: (route) => set({ route }),
      setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
      setDetailPaneCollapsed: (detailPaneCollapsed) => set({ detailPaneCollapsed }),
      setDetailSidebarCollapsed: (detailSidebarCollapsed) => set({ detailSidebarCollapsed }),
      setPaneWidth: (pane, width) =>
        set((state) => ({
          paneWidths: { ...state.paneWidths, [pane]: width },
        })),
    }),
    {
      name: "link-world-ui",
      partialize: (state) => ({ 
        paneWidths: state.paneWidths, 
        sidebarCollapsed: state.sidebarCollapsed,
        detailPaneCollapsed: state.detailPaneCollapsed,
        detailSidebarCollapsed: state.detailSidebarCollapsed
      }),
    }
  )
);

