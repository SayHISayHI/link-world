import { create } from "zustand";
import { initialRoute, type AppRoute } from "../app/routes";

interface UiStore {
  route: AppRoute;
  sidebarCollapsed: boolean;
  setRoute: (route: AppRoute) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
}

export const useUiStore = create<UiStore>((set) => ({
  route: initialRoute,
  sidebarCollapsed: false,
  setRoute: (route) => set({ route }),
  setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
}));

