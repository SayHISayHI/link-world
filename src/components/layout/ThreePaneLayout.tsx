import { useState, useRef, useEffect } from "react";
import type { ReactNode } from "react";
import { useUiStore } from "../../store/uiStore";
import { Resizer } from "./Resizer";

interface ThreePaneLayoutProps {
  topBar?: ReactNode;
  sidebar: ReactNode;
  list: ReactNode;
  detail: ReactNode;
}

export function ThreePaneLayout({ topBar, sidebar, list, detail }: ThreePaneLayoutProps) {
  const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const detailPaneCollapsed = useUiStore((s) => s.detailPaneCollapsed);
  const storeWidths = useUiStore((s) => s.paneWidths);
  const setStoreWidth = useUiStore((s) => s.setPaneWidth);

  const [sidebarWidth, setSidebarWidth] = useState(storeWidths.sidebar);
  const [listWidth, setListWidth] = useState(storeWidths.list);
  const initialWidthsRef = useRef({ sidebar: storeWidths.sidebar, list: storeWidths.list });

  useEffect(() => {
    setSidebarWidth(storeWidths.sidebar);
    setListWidth(storeWidths.list);
  }, [storeWidths.sidebar, storeWidths.list]);

  const handleSidebarDragStart = () => {
    initialWidthsRef.current.sidebar = sidebarWidth;
  };
  
  const handleSidebarDrag = (deltaX: number) => {
    setSidebarWidth(Math.max(160, Math.min(400, initialWidthsRef.current.sidebar + deltaX)));
  };
  
  const handleSidebarDragEnd = () => {
    setStoreWidth("sidebar", sidebarWidth);
  };

  const handleListDragStart = () => {
    initialWidthsRef.current.list = listWidth;
  };

  const handleListDrag = (deltaX: number) => {
    setListWidth(Math.max(250, Math.min(600, initialWidthsRef.current.list + deltaX)));
  };

  const handleListDragEnd = () => {
    setStoreWidth("list", listWidth);
  };

  return (
    <div className="flex h-screen w-full flex-col overflow-hidden">
      {topBar}
      <div className="flex min-h-0 flex-1 border-border">
        <aside 
          className="relative shrink-0 border-r border-border bg-surface"
          style={{ width: sidebarCollapsed ? 56 : sidebarWidth }}
        >
          {sidebar}
          {!sidebarCollapsed && (
            <Resizer 
              className="absolute right-0 top-0 bottom-0 translate-x-1/2"
              onDragStart={handleSidebarDragStart}
              onDrag={handleSidebarDrag} 
              onDragEnd={handleSidebarDragEnd} 
            />
          )}
        </aside>
        
        <section 
          className="relative shrink-0 border-r border-border bg-background"
          style={{ width: detailPaneCollapsed ? "100%" : listWidth, flex: detailPaneCollapsed ? "1 1 0%" : "0 0 auto" }}
        >
          {list}
          {!detailPaneCollapsed && (
            <Resizer 
              className="absolute right-0 top-0 bottom-0 translate-x-1/2"
              onDragStart={handleListDragStart}
              onDrag={handleListDrag} 
              onDragEnd={handleListDragEnd} 
            />
          )}
        </section>
        
        <section 
          className={`min-w-0 bg-surface ${
            detailPaneCollapsed ? "w-0 opacity-0 overflow-hidden border-none" : "flex-1 opacity-100"
          }`}
        >
          <div className="w-full h-full min-w-[300px]">
            {detail}
          </div>
        </section>
      </div>
    </div>
  );
}

