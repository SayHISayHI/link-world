import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { useI18n } from "../../i18n";

type WindowAction = "close" | "minimize" | "toggleMaximize";

async function runWindowAction(action: WindowAction) {
  if (!isTauri()) return;

  try {
    await getCurrentWindow()[action]();
  } catch (error) {
    console.error(`Window action ${action} failed.`, error);
  }
}

export function WindowControls() {
  const { t } = useI18n();

  return (
    <div
      role="group"
      aria-label={t("Window controls")}
      className="ml-1 flex h-full items-stretch border-l border-border/50"
    >
      <button
        type="button"
        className="inline-flex h-full w-12 items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent"
        onClick={() => void runWindowAction("minimize")}
        title={t("Minimize window")}
        aria-label={t("Minimize window")}
      >
        <Minus className="h-4 w-4" strokeWidth={1.5} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="inline-flex h-full w-12 items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent"
        onClick={() => void runWindowAction("toggleMaximize")}
        title={t("Maximize or restore window")}
        aria-label={t("Maximize or restore window")}
      >
        <Square className="h-3 w-3" strokeWidth={1.5} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="inline-flex h-full w-12 items-center justify-center text-muted-foreground hover:bg-red-600 hover:text-white focus-visible:z-10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-red-300"
        onClick={() => void runWindowAction("close")}
        title={t("Close window")}
        aria-label={t("Close window")}
      >
        <X className="h-4 w-4" strokeWidth={1.5} aria-hidden="true" />
      </button>
    </div>
  );
}
