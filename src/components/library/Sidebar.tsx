import { useState, type FormEvent } from "react";
import { useUiStore } from "../../store/uiStore";
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
  PanelLeftClose,
  PanelLeftOpen,
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
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useUiStore((s) => s.setSidebarCollapsed);
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
    <div className={`flex h-full flex-col py-3 ${sidebarCollapsed ? "overflow-hidden px-0" : "overflow-y-auto px-3"}`}>

      <nav className={`mt-2 space-y-5 ${sidebarCollapsed ? "w-full" : ""}`} aria-label="Knowledge library">
        <NavigationSection
          label="Library"
          items={navigation?.systemViews ?? []}
          activeView={activeView}
          loading={loading}
          collapsed={sidebarCollapsed}
          onSelect={(view) => onNavigate({ name: "library", view })}
        />

        <section>
          {!sidebarCollapsed && (
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
          )}
          {sidebarCollapsed && (
            <div className="flex h-7 items-center justify-center">
              <h2 className="text-xs font-bold uppercase text-muted-foreground tracking-tighter">COL</h2>
            </div>
          )}
          
          {creating && !sidebarCollapsed ? (
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
            collapsed={sidebarCollapsed}
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
        </section>

        <NavigationSection
          label="Topics"
          items={navigation?.topics ?? []}
          activeView={activeView}
          collapsed={sidebarCollapsed}
          onSelect={(view) => onNavigate({ name: "library", view })}
        />
        <NavigationSection
          label="Smart Views"
          items={navigation?.smartViews ?? []}
          activeView={activeView}
          collapsed={sidebarCollapsed}
          onSelect={(view) => onNavigate({ name: "library", view })}
          onAdd={onCreateSmartView}
        />
      </nav>

      {error && !sidebarCollapsed ? (
        <div className="mt-4 border-l-2 border-red-400 px-2 text-xs text-red-700">
          <p className="font-medium">Navigation unavailable</p>
          <p className="mt-1">{error.message}</p>
        </div>
      ) : null}

      <div className={`mt-auto pt-4 flex ${sidebarCollapsed ? "flex-col space-y-2 w-full" : "items-center gap-1"}`}>
        <Button
          variant={route.name === "settings" ? "secondary" : "ghost"}
          className={sidebarCollapsed ? "h-9 w-9 mx-auto justify-center px-0" : "flex-1 h-9 justify-start"}
          onClick={() => onNavigate({ name: "settings", panel: "models" })}
          title="Settings"
        >
          <Settings className="h-4 w-4 shrink-0" aria-hidden="true" />
          {!sidebarCollapsed && <span className="truncate">Settings</span>}
        </Button>
        <Button
          variant="ghost"
          className={sidebarCollapsed ? "h-9 w-9 mx-auto justify-center px-0" : "shrink-0 w-9 h-9 px-0"}
          onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
          title={sidebarCollapsed ? "Expand sidebar (Ctrl+B)" : "Collapse sidebar (Ctrl+B)"}
        >
          {sidebarCollapsed ? (
            <PanelLeftOpen className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          ) : (
            <PanelLeftClose className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          )}
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
  collapsed = false,
  onSelect,
  onAdd,
}: {
  label: string;
  items: NavigationItem[];
  activeView?: LibraryViewRef;
  loading?: boolean;
  collapsed?: boolean;
  onSelect: (view: LibraryViewRef) => void;
  onAdd?: () => void;
}) {
  return (
    <section>
      {!collapsed && (
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
      )}
      {collapsed && (
        <div className="flex h-7 items-center justify-center">
          <h2 className="text-xs font-bold uppercase text-muted-foreground tracking-tighter">
            {label === "Smart Views" ? "S.V" : label.slice(0, 3)}
          </h2>
        </div>
      )}
      
      {loading && items.length === 0 ? (
        <p className={`py-1 text-xs text-muted-foreground ${collapsed ? "text-center" : "px-2"}`}>...</p>
      ) : (
        <NavigationItems items={items} activeView={activeView} onSelect={onSelect} collapsed={collapsed} />
      )}
    </section>
  );
}

function NavigationItems({
  items,
  activeView,
  collapsed = false,
  onSelect,
  renderActions,
}: {
  items: NavigationItem[];
  activeView?: LibraryViewRef;
  collapsed?: boolean;
  onSelect: (view: LibraryViewRef) => void;
  renderActions?: (item: NavigationItem) => React.ReactNode;
}) {
  return (
    <div className="space-y-0.5">
      {items.map((item) => {
        const Icon = icons[item.iconKey ?? ""] ?? Folder;
        const active = activeView?.kind === item.kind && activeView.id === item.id;
        return (
          <div key={`${item.kind}:${item.id}`} className="group relative flex items-center w-full">
            <button
              type="button"
              className={`flex min-w-0 items-center gap-2 rounded-md text-sm transition-all duration-200 ${
                collapsed ? "h-9 w-9 justify-center mx-auto" : "h-9 flex-1 px-2 text-left"
              } ${
                active
                  ? "bg-accent/10 font-medium text-accent"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground"
              }`}
              onClick={() => onSelect({ kind: item.kind, id: item.id })}
              aria-current={active ? "page" : undefined}
              title={collapsed ? item.label : undefined}
            >
              <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
              {!collapsed && (
                <>
                  <span className="min-w-0 flex-1 truncate">{item.label}</span>
                  <span
                    className={`text-[11px] tabular-nums text-muted-foreground ${
                      renderActions ? "group-hover:opacity-0 group-focus-within:opacity-0" : ""
                    }`}
                  >
                    {item.count}
                  </span>
                </>
              )}
            </button>
            {renderActions && !collapsed ? (
              <div className="absolute right-1 flex items-center opacity-0 group-hover:opacity-100 group-focus-within:opacity-100">
                {renderActions(item)}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}