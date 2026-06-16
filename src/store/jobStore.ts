import { create } from "zustand";

interface JobStore {
  activeJobCount: number;
  setActiveJobCount: (count: number) => void;
}

export const useJobStore = create<JobStore>((set) => ({
  activeJobCount: 0,
  setActiveJobCount: (activeJobCount) => set({ activeJobCount }),
}));

