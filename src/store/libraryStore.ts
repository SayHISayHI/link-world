import { create } from "zustand";
import type { KnowledgeObject, KnowledgeObjectDetail } from "../types/api";

interface LibraryStore {
  selectedObjectId?: string;
  objects: KnowledgeObject[];
  selectedDetail?: KnowledgeObjectDetail;
  selectObject: (objectId: string) => void;
  setObjects: (objects: KnowledgeObject[]) => void;
  setSelectedDetail: (detail?: KnowledgeObjectDetail) => void;
}

export const useLibraryStore = create<LibraryStore>((set) => ({
  selectedObjectId: undefined,
  objects: [],
  selectedDetail: undefined,
  selectObject: (selectedObjectId) => set({ selectedObjectId }),
  setObjects: (objects) =>
    set((state) => ({
      objects,
      selectedObjectId:
        state.selectedObjectId && objects.some((object) => object.id === state.selectedObjectId)
          ? state.selectedObjectId
          : objects[0]?.id,
    })),
  setSelectedDetail: (selectedDetail) => set({ selectedDetail }),
}));
