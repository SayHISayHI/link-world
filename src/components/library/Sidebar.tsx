import {
  AlertCircle,
  BookOpen,
  FileText,
  Github,
  Inbox,
  Settings,
  Sparkles,
} from "lucide-react";
import type { AppRoute } from "../../app/routes";
import { Button } from "../ui/button";

interface SidebarProps {
  route: AppRoute;
  onNavigate: (route: AppRoute) => void;
}

const navItems: Array<{
  label: string;
  filter: string;
  icon: typeof BookOpen;
}> = [
  { label: "All", filter: "all", icon: BookOpen },
  { label: "Inbox", filter: "inbox", icon: Inbox },
  { label: "Articles", filter: "article", icon: FileText },
  { label: "GitHub", filter: "github_repo", icon: Github },
  { label: "Prompts", filter: "prompt", icon: Sparkles },
  { label: "Failed", filter: "failed", icon: AlertCircle },
];

export function Sidebar({ route, onNavigate }: SidebarProps) {
  return (
    <div className="flex h-full flex-col overflow-y-auto p-3">
      <div className="px-2 py-3">
        <div className="text-sm font-semibold tracking-normal">Link World</div>
        <div className="mt-1 text-xs text-muted-foreground">Local knowledge workspace</div>
      </div>
      <nav className="mt-3 space-y-1" aria-label="Library categories">
        {navItems.map((item) => {
          const active = route.name === "library" && (route.filter ?? "all") === item.filter;
          return (
            <Button
              key={item.filter}
              variant={active ? "secondary" : "ghost"}
              className="w-full justify-start"
              onClick={() => onNavigate({ name: "library", filter: item.filter })}
            >
              <item.icon className="h-4 w-4" aria-hidden="true" />
              {item.label}
            </Button>
          );
        })}
      </nav>
      <div className="mt-auto">
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

