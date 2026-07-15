import { useEffect } from "react";
import { useUiStore } from "../store/uiStore";
import { AppShell } from "../components/layout/AppShell";
import { WindowTitleBar } from "../components/layout/WindowTitleBar";
import { StartupRecoveryScreen } from "../components/recovery/StartupRecoveryScreen";
import { LibraryShellContainer } from "../containers/LibraryShellContainer";
import { useStartupStatus } from "../hooks/commands/useStartupStatus";
import { useI18n } from "../i18n";

export function App() {
  const { t } = useI18n();
  const { error, loading, loadStartupStatus, status } = useStartupStatus();
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed);

  const detailPaneCollapsed = useUiStore((s) => s.detailPaneCollapsed);
  const setDetailPaneCollapsed = useUiStore((s) => s.setDetailPaneCollapsed);

  useEffect(() => {
    void loadStartupStatus();
  }, [loadStartupStatus]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Toggle sidebar on Cmd+B or Ctrl+B, and detail pane on Cmd+Alt+B or Ctrl+Alt+B
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault(); // Prevent default browser bookmark shortcut
        if (e.altKey) {
          setDetailPaneCollapsed(!detailPaneCollapsed);
        } else {
          setSidebarCollapsed(!sidebarCollapsed);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sidebarCollapsed, setSidebarCollapsed, detailPaneCollapsed, setDetailPaneCollapsed]);

  if (status?.mode === "recovery") {
    return <StartupRecoveryScreen status={status} />;
  }

  if (loading && !error) {
    return (
      <AppShell>
        <div className="flex h-screen flex-col overflow-hidden bg-background">
          <WindowTitleBar />
          <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
            {t("Starting Node Tide...")}
          </div>
        </div>
      </AppShell>
    );
  }

  return <LibraryShellContainer />;
}

