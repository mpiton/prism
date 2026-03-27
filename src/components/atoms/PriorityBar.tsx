import type { ReactElement } from "react";
import type { Priority } from "../../lib/types";

interface PriorityBarProps {
  readonly priority: Priority;
}

const PRIORITY_CONFIG: Record<Priority, { height: string; color: string }> = {
  critical: { height: "h-full", color: "bg-red" },
  high: { height: "h-3/4", color: "bg-orange" },
  medium: { height: "h-1/2", color: "bg-blue" },
  low: { height: "h-1/4", color: "bg-dim" },
};

export function PriorityBar({ priority }: PriorityBarProps): ReactElement {
  const { height, color } = PRIORITY_CONFIG[priority];
  return (
    <div
      role="img"
      aria-label={`Priority: ${priority}`}
      className={`w-1 rounded-full ${height} ${color}`}
    />
  );
}
