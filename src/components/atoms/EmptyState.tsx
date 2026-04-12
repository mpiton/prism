import type { ReactElement } from "react";
import { FOCUS_RING } from "../../lib/a11y";

interface EmptyStateCTA {
  readonly text: string;
  readonly onClick: () => void;
}

interface EmptyStateProps {
  readonly message: string;
  readonly icon?: string;
  readonly cta?: EmptyStateCTA;
}

export function EmptyState({ message, icon, cta }: EmptyStateProps): ReactElement {
  return (
    <div className="flex flex-col items-center justify-center gap-1 py-8 text-center">
      <div role="status" className="flex flex-col items-center gap-1">
        {icon && <span aria-hidden="true" className="text-lg">{icon}</span>}
        <span className="text-sm text-dim">{message}</span>
      </div>
      {cta && (
        <button
          type="button"
          onClick={cta.onClick}
          className={`${FOCUS_RING} mt-2 inline-flex min-h-11 items-center rounded border border-border px-3 text-xs font-medium text-dim transition-colors hover:border-accent hover:text-white`}
        >
          {cta.text}
        </button>
      )}
    </div>
  );
}
