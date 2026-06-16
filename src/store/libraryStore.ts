import { create } from "zustand";
import type { KnowledgeObjectSummary } from "../types/api";

interface LibraryStore {
  selectedObjectId?: string;
  objects: KnowledgeObjectSummary[];
  selectObject: (objectId: string) => void;
}

export const useLibraryStore = create<LibraryStore>((set) => ({
  selectedObjectId: "seed-article",
  objects: [
    {
      id: "seed-article",
      title: "Local-first knowledge systems",
      type: "article",
      lifecycleStatus: "parsed",
      summary: "A saved article ready for local analysis.",
    },
    {
      id: "seed-repo",
      title: "example/dev-runner",
      type: "github_repo",
      lifecycleStatus: "captured",
      summary: "Repository metadata capture queued.",
    },
    {
      id: "seed-prompt",
      title: "Code review prompt",
      type: "prompt",
      lifecycleStatus: "failed",
      summary: "Needs retry after parser fallback.",
    },
  ],
  selectObject: (selectedObjectId) => set({ selectedObjectId }),
}));

