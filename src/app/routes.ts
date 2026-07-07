import type { LibraryFilters, LibraryViewRef } from "../types/api";

export type AppRoute =
  | { name: "library"; view?: LibraryViewRef; filters?: LibraryFilters; objectId?: string }
  | { name: "search"; query?: string }
  | {
      name: "settings";
      panel?:
        | "models"
        | "privacy"
        | "capture"
        | "plugins"
        | "storage"
        | "diagnostics"
        | "about";
    }
  | { name: "evaluation"; objectId: string; runId?: string };

export const allLibraryView: LibraryViewRef = { kind: "system", id: "all" };

export const emptyLibraryFilters: LibraryFilters = {
  objectTypes: [],
  lifecycleStatuses: [],
  tagIds: [],
  privacyLevels: [],
};

export const initialRoute: AppRoute = {
  name: "library",
  view: { kind: "system", id: "inbox" },
  filters: emptyLibraryFilters,
};