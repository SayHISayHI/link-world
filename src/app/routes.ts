export type AppRoute =
  | { name: "library"; filter?: string; objectId?: string }
  | { name: "search"; query?: string }
  | { name: "settings"; panel?: "models" | "privacy" | "capture" | "plugins" | "storage" | "diagnostics" | "about" }
  | { name: "evaluation"; objectId: string; runId?: string };

export const initialRoute: AppRoute = { name: "library", filter: "all" };

