import { Activity, RefreshCw } from "lucide-react";
import { Button } from "../ui/button";
import type { KnowledgeObjectSummary, PingResponse } from "../../types/api";
import type { AppUiError } from "../../lib/errors";

interface ObjectDetailProps {
  object?: KnowledgeObjectSummary;
  pingData?: PingResponse;
  pingError?: AppUiError;
  pingLoading: boolean;
  onPing: () => void;
}

export function ObjectDetail({ object, pingData, pingError, pingLoading, onPing }: ObjectDetailProps) {
  if (!object) {
    return (
      <div className="flex h-screen items-center justify-center text-sm text-muted-foreground">
        Select an item to inspect.
      </div>
    );
  }

  return (
    <div className="flex h-screen min-w-0 flex-col">
      <header className="flex h-14 items-center justify-between border-b border-border px-5">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold">{object.title}</h2>
          <p className="text-xs text-muted-foreground">{object.type}</p>
        </div>
        <Button onClick={onPing} disabled={pingLoading} title="Ping backend">
          <RefreshCw className="h-4 w-4" aria-hidden="true" />
          Ping
        </Button>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_320px]">
        <article className="overflow-y-auto p-6">
          <div className="max-w-3xl">
            <div className="mb-4 flex items-center gap-2 text-xs text-muted-foreground">
              <Activity className="h-4 w-4" aria-hidden="true" />
              Phase 1 scaffold
            </div>
            <h3 className="text-lg font-semibold">Parsed document preview</h3>
            <p className="mt-3 text-sm leading-6 text-muted-foreground">
              This scaffold keeps the frontend separated from Tauri commands. The right panel calls the
              backend through a typed hook and displays the structured IPC response.
            </p>
          </div>
        </article>
        <aside className="border-l border-border bg-background p-4">
          <h3 className="text-sm font-semibold">Backend IPC</h3>
          <div className="mt-3 rounded-md border border-border bg-surface p-3 text-xs leading-5">
            {pingLoading ? <p className="text-muted-foreground">Pinging backend...</p> : null}
            {pingData ? (
              <div>
                <p className="font-medium">{pingData.message}</p>
                <p className="mt-1 text-muted-foreground">Backend {pingData.backendVersion}</p>
              </div>
            ) : null}
            {pingError ? (
              <div>
                <p className="font-medium text-red-700">{pingError.title}</p>
                <p className="mt-1 text-muted-foreground">{pingError.message}</p>
              </div>
            ) : null}
            {!pingLoading && !pingData && !pingError ? (
              <p className="text-muted-foreground">Use Ping to validate Tauri IPC.</p>
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}

