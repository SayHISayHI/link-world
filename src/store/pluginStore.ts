import { create } from "zustand";

interface PluginStore {
  enabledCount: number;
  setEnabledCount: (count: number) => void;
}

export const usePluginStore = create<PluginStore>((set) => ({
  enabledCount: 0,
  setEnabledCount: (enabledCount) => set({ enabledCount }),
}));

