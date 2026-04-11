import type { ReactElement } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import type { WorkspaceState } from "../../lib/types";

interface WsBadgeProps {
  readonly state?: WorkspaceState;
  readonly loading?: boolean;
  readonly onClick?: () => void;
  readonly ariaLabel?: string;
}

const LABEL_MAP: Record<Exclude<WorkspaceState, "archived">, string> = {
  active: "resume",
  suspended: "wake",
};

export function WsBadge({ state, loading, onClick, ariaLabel }: WsBadgeProps): ReactElement | null {
  if (state === "archived") {
    return null;
  }

  const label = loading
    ? "cloning…"
    : state
      ? LABEL_MAP[state as Exclude<WorkspaceState, "archived">]
      : "open";

  return (
    <button
      type="button"
      onClick={loading ? undefined : onClick}
      disabled={loading}
      aria-label={ariaLabel}
      className={`${FOCUS_RING} rounded border border-accent/30 px-2 py-2 text-xs text-accent ${loading ? "animate-pulse opacity-60" : "hover:bg-accent/10"}`}
    >
      {label}
    </button>
  );
}
