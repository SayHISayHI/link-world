import { PRODUCT_DISPLAY_NAME } from "../../config/brand";
import { NativeWindowControlsInset, WindowControls } from "./WindowControls";

export function WindowTitleBar() {
  return (
    <header
      data-testid="window-titlebar"
      data-tauri-drag-region="deep"
      className="flex h-12 shrink-0 select-none items-center border-b border-border/60 bg-surface"
    >
      <div className="flex h-full min-w-0 flex-1 items-center px-4">
        <NativeWindowControlsInset />
        <h1 className="truncate text-sm font-semibold tracking-normal text-foreground">
          {PRODUCT_DISPLAY_NAME}
        </h1>
      </div>
      <WindowControls />
    </header>
  );
}
