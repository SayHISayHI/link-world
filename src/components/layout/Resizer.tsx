import { useEffect, useRef, useState } from "react";
import { cn } from "../../lib/cn";

interface ResizerProps {
  onDragStart?: () => void;
  onDrag: (deltaX: number) => void;
  onDragEnd?: () => void;
  className?: string;
}

export function Resizer({ onDragStart, onDrag, onDragEnd, className }: ResizerProps) {
  const [isDragging, setIsDragging] = useState(false);
  const startXRef = useRef<number>(0);

  useEffect(() => {
    if (!isDragging) return;

    const handlePointerMove = (e: PointerEvent) => {
      // Calculate delta from the starting position
      const deltaX = e.clientX - startXRef.current;
      onDrag(deltaX);
    };

    const handlePointerUp = () => {
      setIsDragging(false);
      onDragEnd?.();
      document.body.style.cursor = "auto";
      document.body.style.userSelect = "auto";
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    
    // Prevent text selection while dragging
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isDragging, onDrag, onDragEnd]);

  const handlePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    startXRef.current = e.clientX;
    setIsDragging(true);
    onDragStart?.();
  };

  return (
    <div
      className={cn(
        "group relative flex w-1.5 cursor-col-resize flex-col items-center justify-center transition-colors hover:bg-accent/30 z-50",
        isDragging && "bg-accent/30",
        className
      )}
      onPointerDown={handlePointerDown}
    >
      <div className={cn(
        "w-[1px] h-full bg-transparent group-hover:bg-accent/50 transition-colors",
        isDragging && "bg-accent/50"
      )} />
    </div>
  );
}
