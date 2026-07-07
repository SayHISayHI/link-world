import { useState, type FormEvent } from "react";
import {
  AlertCircle,
  BookOpen,
  Check,
  Filter,
  Folder,
  Inbox,
  Pencil,
  Plus,
  Settings,
  Tag,
  Trash2,
  X,
  type LucideIcon,
} from "lucide-react";
import type { AppRoute } from "../../app/routes";
import type { AppUiError } from "../../lib/errors";
import type { LibraryNavigation, LibraryViewRef, NavigationItem } from "../../types/api";
import { Button } from "../ui/button";

interface SidebarProps {
  route: AppRoute;
  navigation?: LibraryNavigation;
  loading: boolean;
  mutationLoading: boolean;
  error?: AppUiError;
  onNavigate: (route: AppRoute) => void;
  onCreateCollection: (name: string) => void;
  onCreateSmartView: () => void;
  onRenameCollection: (item: NavigationItem) => void;
  onArchiveCollection: (item: NavigationItem) => void;
}

const icons: Record<string, LucideIcon> = {
  "alert-circle": AlertCircle,
  filter: Filter,
  folder: Folder,
  inbox: Inbox,
  library: BookOpen,
  tag: Tag,
};

export function Sidebar({
  route,
  navigation,
  loading,
  mutationLoading,
  error,
  onNavigate,
  onCreateCollection,
  onCreateSmartView,
  onRenameCollection,
  onArchiveCollection,
}: SidebarProps) {
  const [creating, setCreating] = useState(false);
  const [collectionName, setCollectionName] = useState("");
  const activeView = route.name === "library" ? route.view : undefined;

  const handleCreate = (event: FormEvent) => {
    event.preventDefault();
    const name = collectionName.trim();
    if (!name) return;
    onCreateCollection(name);
    setCollectionName("");
    setCreating(false);
  };

  return (
    <div className="flex h-full flex-col overflow-y-auto px-3 py-3">
      <div className="px-2 py-3">
        <div className="text-sm font-semibold tracking-normal">Link World</div>
        <div className="mt-1 text-xs text-muted-foreground">Local knowledge workspace</div>
      </div>

      <nav className="mt-2 space-y-5" aria-label="Knowledge library">
        <NavigationSection
          label="Library"
          items={navigation?.systemViews ?? []}
          activeView={activeView}
          loading={loading}
          onSelect={(view) => onNavigate({ name: "library", view })}
        />

        <section>
          <div className="flex h-7 items-center justify-between px-2">
            <h2 className="text-[11px] font-semibold uppercase text-muted-foreground">Collections</h2>
            <button
              type="button"
              className="flex h-6 w-6 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
              onClick={() => setCreating(true)}
              title="New collection"
              aria-label="New collection"
            >
              <Plus className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </div>
          {creating ? (
            <form className="mt-1 flex items-center gap-1 px-1" onSubmit={handleCreate}>
              <input
                autoFocus
                value={collectionName}
                maxLength={80}
                onChange={(event) => setCollectionName(event.target.value)}
                className="h-8 min-w-0 flex-1 rounded-sm border border-border bg-surface px-2 text-xs outline-none focus:border-accent"
                placeholder="Collection name"
                aria-label="Collection name"
              />
              <button
                type="submit"
                disabled={mutationLoading || !collectionName.trim()}
                className="flex h-8 w-8 items-center justify-center rounded-sm text-accent hover:bg-muted disabled:opacity-40"
                title="Create collection"
                aria-label="Create collection"
              >
                <Check className="h-4 w-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                className="flex h-8 w-8 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted"
                onClick={() => {
                  setCreating(false);
                  setCollectionName("");
                }}
                title="Cancel"
                aria-label="Cancel new collection"
              >
                <X className="h-4 w-4" aria-hidden="true" />
              </button>
            </form>
          ) : null}
          <NavigationItems
            items={navigation?.collections ?? []}
            activeView={activeView}
            onSelect={(view) => onNavigate({ name: "library", view })}
            renderActions={(item) => (
              <>
                <button
                  type="button"
                  className="flex h-6 w-6 items-center justify-center rounded-sm text-muted-foreground hover:bg-surface hover:text-foreground"
                  onClick={(event) => {
                    event.stopPropagation();
                    onRenameCollection(item);
                  }}
                  title={`Rename ${item.label}`}
                  aria-label={`Rename ${item.label}`}
                >
                  <Pencil className="h-3 w-3" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className="flex h-6 w-6 items-center justify-center rounded-sm text-muted-foreground hover:bg-red-50 hover:text-red-700"
                  onClick={(event) => {
                    event.stopPropagation();
                    onArchiveCollection(item);
                  }}
                  title={`Archive ${item.label}`}
                  aria-label={`Archive ${item.label}`}
                >
                  <Trash2 className="h-3 w-3" aria-hidden="true" />
                </button>
              </>
            )}
          />
          {!loading && navigation?.collections.length === 0 && !creating ? (
            <p className="px-2 py-1 text-xs text-muted-foreground">No collections</p>
          ) : null}
        </section>

        <NavigationSection
          label="Topics"
          items={navigation?.topics ?? []}
          activeView={activeView}
          onSelect={(view) => onNavigate({ name: "library", view })}
        />
        <NavigationSection
          label="Smart Views"
          items={navigation?.smartViews ?? []}
          activeView={activeView}
          onSelect={(view) => onNavigate({ name: "library", view })}
          onAdd={onCreateSmartView}
        />
      </nav>

      {error ? (
        <div className="mt-4 border-l-2 border-red-400 px-2 text-xs text-red-700">
          <p className="font-medium">Navigation unavailable</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      <div className="mt-auto pt-4">
        <Button
          variant={route.name === "settings" ? "secondary" : "ghost"}
          className="w-full justify-start"
          onClick={() => onNavigate({ name: "settings", panel: "models" })}
        >
          <Settings className="h-4 w-4" aria-hidden="true" />
          Settings
        </Button>
      </div>
    </div>
  );
}

function NavigationSection({
  label,
  items,
  activeView,
  loading = false,
  onSelect,
  onAdd,
}: {
  label: string;
  items: NavigationItem[];
  activeView?: LibraryViewRef;
  loading?: boolean;
  onSelect: (view: LibraryViewRef) => void;
  onAdd?: () => void;
}) {
  return (
    <section>
      <div className="flex h-7 items-center justify-between px-2">
        <h2 className="text-[11px] font-semibold uppercase text-muted-foreground">{label}</h2>
        {onAdd ? (
          <button
            type="button"
            className="flex h-6 w-6 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
            onClick={onAdd}
            title={"New " + label.toLowerCase().replace(/s$/, "")}
            aria-label={"New " + label.toLowerCase().replace(/s$/, "")}
          >
            <Plus className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        ) : null}
      </div>
      {loading && items.length === 0 ? (
        <p className="px-2 py-1 text-xs text-muted-foreground">Loading...</p>
      ) : (
        <NavigationItems items={items} activeView={activeView} onSelect={onSelect} />
      )}
    </section>
  );
}

function NavigationItems({
  items,
  activeView,
  onSelect,
  renderActions,
}: {
  items: NavigationItem[];
  activeView?: LibraryViewRef;
  onSelect: (view: LibraryViewRef) => void;
  renderActions?: (item: NavigationItem) => React.ReactNode;
}) {
  return (
    <div className="space-y-0.5">
      {items.map((item) => {
        const Icon = icons[item.iconKey ?? ""] ?? Folder;
        const active = activeView?.kind === item.kind && activeView.id === item.id;
        return (
          <div key={`${item.kind}:${item.id}`} className="group relative flex items-center">
            <button
              type="button"
              className={`flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left text-sm transition-colors ${
                active
                  ? "bg-muted font-medium text-foreground"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground"
              }`}
              onClick={() => onSelect({ kind: item.kind, id: item.id })}
              aria-current={active ? "page" : undefined}
            >
              <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate">{item.label}</span>
              <span
                className={`text-[11px] tabular-nums text-muted-foreground ${
                  renderActions ? "group-hover:opacity-0 group-focus-within:opacity-0" : ""
                }`}
              >
                {item.count}
              </span>
            </button>
            {renderActions ? (
              <div className="absolute right-1 flex items-center bg-muted opacity-0 group-hover:opacity-100 group-focus-within:opacity-100">
                {renderActions(item)}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}