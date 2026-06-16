import type { KnowledgeObjectSummary } from "../../types/api";
import { formatRelativeStatus } from "../../lib/formatting";
import { cn } from "../../lib/cn";

interface ObjectListProps {
  objects: KnowledgeObjectSummary[];
  selectedObjectId?: string;
  onSelectObject: (objectId: string) => void;
}

export function ObjectList({ objects, selectedObjectId, onSelectObject }: ObjectListProps) {
  return (
    <div className="h-screen overflow-y-auto p-3">
      <div className="mb-3 px-1">
        <h1 className="text-base font-semibold">Inbox</h1>
        <p className="mt-1 text-xs text-muted-foreground">Captured items ready for processing.</p>
      </div>
      <div className="space-y-2">
        {objects.map((object) => (
          <button
            key={object.id}
            className={cn(
              "w-full rounded-md border border-border bg-surface p-3 text-left transition-colors hover:bg-muted",
              selectedObjectId === object.id && "border-accent bg-accent/5",
            )}
            onClick={() => onSelectObject(object.id)}
            type="button"
          >
            <div className="flex items-center justify-between gap-3">
              <div className="truncate text-sm font-medium">{object.title}</div>
              <span className="shrink-0 rounded-sm bg-muted px-2 py-1 text-[11px] text-muted-foreground">
                {formatRelativeStatus(object.lifecycleStatus)}
              </span>
            </div>
            {object.summary ? (
              <p className="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">{object.summary}</p>
            ) : null}
          </button>
        ))}
      </div>
    </div>
  );
}

