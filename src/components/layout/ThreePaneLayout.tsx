import type { ReactNode } from "react";

interface ThreePaneLayoutProps {
  sidebar: ReactNode;
  list: ReactNode;
  detail: ReactNode;
}

export function ThreePaneLayout({ sidebar, list, detail }: ThreePaneLayoutProps) {
  return (
    <div className="grid min-h-screen grid-cols-[232px_360px_minmax(0,1fr)] border-border">
      <aside className="border-r border-border bg-surface">{sidebar}</aside>
      <section className="border-r border-border bg-background">{list}</section>
      <section className="min-w-0 bg-surface">{detail}</section>
    </div>
  );
}

