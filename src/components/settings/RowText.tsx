import type { ReactNode } from "react";

export function RowText({ title, description }: { title: ReactNode; description: ReactNode }) {
  return (
    <div className="flex-1">
      <div className="text-sm font-semibold">{title}</div>
      <div className="text-xs text-muted-foreground">{description}</div>
    </div>
  );
}
