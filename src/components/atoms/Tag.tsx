import type { ReactElement, ReactNode } from "react";

interface TagProps {
  readonly children: ReactNode;
  readonly className?: string;
}

export function Tag({ children, className = "" }: TagProps): ReactElement {
  return (
    <span className={`text-xs font-medium uppercase tracking-wide ${className || "text-dim"}`}>
      {children}
    </span>
  );
}
