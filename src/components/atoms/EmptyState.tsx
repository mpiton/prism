import type { ReactElement } from "react";

interface EmptyStateProps {
  readonly message: string;
  readonly icon?: string;
}

export function EmptyState({ message, icon }: EmptyStateProps): ReactElement {
  return (
    <div role="status" className="flex flex-col items-center justify-center gap-1 py-8 text-center">
      {icon && <span className="text-lg">{icon}</span>}
      <span className="text-sm text-dim">{message}</span>
    </div>
  );
}
