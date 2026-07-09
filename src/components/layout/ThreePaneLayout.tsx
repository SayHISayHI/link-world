import type { ReactNode } from "react";

interface ThreePaneLayoutProps {
  topBar?: ReactNode;
  sidebar: ReactNode;
  list: ReactNode;
  detail: ReactNode;
}

export function ThreePaneLayout({ topBar, sidebar, list, detail }: ThreePaneLayoutProps) {
  return (
    <div className="flex h-screen w-full flex-col overflow-hidden">
      {topBar}
      <div className="flex min-h-0 flex-1 border-border">
        <aside className="w-[232px] shrink-0 border-r border-border bg-surface">{sidebar}</aside>
        <section className="w-[360px] shrink-0 border-r border-border bg-background">{list}</section>
        <section className="min-w-0 flex-1 bg-surface">{detail}</section>
      </div>
    </div>
  );
}

