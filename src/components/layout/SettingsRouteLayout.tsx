import type { ReactNode } from "react";
import { useUiStore } from "../../store/uiStore";
import { COLLAPSED_SIDEBAR_WIDTH } from "./layoutConstants";

interface SettingsRouteLayoutProps {
  topBar: ReactNode;
  sidebar: ReactNode;
  children: ReactNode;
}

export function SettingsRouteLayout({
  topBar,
  sidebar,
  children,
}: SettingsRouteLayoutProps) {
  const sidebarCollapsed = useUiStore((state) => state.sidebarCollapsed);
  const sidebarWidth = useUiStore((state) => state.paneWidths.sidebar);

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden">
      {topBar}
      <div className="flex min-h-0 flex-1 border-t border-border">
        <aside
          data-testid="settings-route-sidebar"
          className="relative shrink-0 border-r border-border bg-surface"
          style={{ width: sidebarCollapsed ? COLLAPSED_SIDEBAR_WIDTH : sidebarWidth }}
        >
          {sidebar}
        </aside>
        <section className="min-w-0 flex-1 overflow-hidden bg-surface">{children}</section>
      </div>
    </div>
  );
}
