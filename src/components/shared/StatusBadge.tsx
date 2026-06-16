interface StatusBadgeProps {
  label: string;
}

export function StatusBadge({ label }: StatusBadgeProps) {
  return <span className="rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">{label}</span>;
}

