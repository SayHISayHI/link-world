import { useEffect } from "react";
import { AppShell } from "../components/layout/AppShell";
import { StartupRecoveryScreen } from "../components/recovery/StartupRecoveryScreen";
import { LibraryShellContainer } from "../containers/LibraryShellContainer";
import { useStartupStatus } from "../hooks/commands/useStartupStatus";

export function App() {
  const { error, loading, loadStartupStatus, status } = useStartupStatus();

  useEffect(() => {
    void loadStartupStatus();
  }, [loadStartupStatus]);

  if (status?.mode === "recovery") {
    return <StartupRecoveryScreen status={status} />;
  }

  if (loading && !error) {
    return (
      <AppShell>
        <div className="flex min-h-screen items-center justify-center text-sm text-muted-foreground">
          Starting Link World...
        </div>
      </AppShell>
    );
  }

  return <LibraryShellContainer />;
}

