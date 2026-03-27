import type { ReactElement } from "react";

interface EmptyStateProps {
  readonly message: string;
}

export function EmptyState({ message }: EmptyStateProps): ReactElement {
  return (
    <div className="flex items-center justify-center py-12 text-center text-sm text-dim">
      {message}
    </div>
  );
}
