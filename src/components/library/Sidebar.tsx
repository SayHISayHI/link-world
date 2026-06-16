import { BookOpen, Inbox, Search, Settings, Sparkles } from "lucide-react";
import { Button } from "../ui/button";

const navItems = [
  { label: "All", icon: BookOpen },
  { label: "Inbox", icon: Inbox },
  { label: "Prompts", icon: Sparkles },
  { label: "Search", icon: Search },
];

export function Sidebar() {
  return (
    <div className="flex h-full flex-col p-3">
      <div className="px-2 py-3">
        <div className="text-sm font-semibold tracking-normal">Link World</div>
        <div className="mt-1 text-xs text-muted-foreground">Local knowledge workspace</div>
      </div>
      <nav className="mt-3 space-y-1">
        {navItems.map((item) => (
          <Button key={item.label} variant="ghost" className="w-full justify-start">
            <item.icon className="h-4 w-4" aria-hidden="true" />
            {item.label}
          </Button>
        ))}
      </nav>
      <div className="mt-auto">
        <Button variant="ghost" className="w-full justify-start">
          <Settings className="h-4 w-4" aria-hidden="true" />
          Settings
        </Button>
      </div>
    </div>
  );
}

