export type AppRoute =
  | { name: "library"; filter?: string; objectId?: string }
  | { name: "search"; query?: string }
  | { name: "settings"; panel?: "models" | "plugins" | "storage" | "diagnostics" }
  | { name: "evaluation"; objectId: string; runId?: string };

export const initialRoute: AppRoute = { name: "library", filter: "all" };

