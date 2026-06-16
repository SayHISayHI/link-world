import { create } from "zustand";

interface SettingsStore {
  modelConfigured: boolean;
  setModelConfigured: (configured: boolean) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  modelConfigured: false,
  setModelConfigured: (modelConfigured) => set({ modelConfigured }),
}));

