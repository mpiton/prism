import type { ReactElement } from "react";
import type { WorkspaceState } from "../../lib/types";

interface WsBadgeProps {
  readonly state?: WorkspaceState;
  readonly onClick?: () => void;
}

const LABEL_MAP: Record<Exclude<WorkspaceState, "archived">, string> = {
  active: "resume",
  suspended: "wake",
};

export function WsBadge({ state, onClick }: WsBadgeProps): ReactElement | null {
  if (state === "archived") {
    return null;
  }

  const label = state ? LABEL_MAP[state as Exclude<WorkspaceState, "archived">] : "open";

  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded border border-accent/30 px-2 py-0.5 text-xs text-accent hover:bg-accent/10"
    >
      {label}
    </button>
  );
}
