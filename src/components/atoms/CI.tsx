import type { ReactElement } from "react";
import type { CiStatus } from "../../lib/types";

interface CIProps {
  readonly status: CiStatus;
}

const STATUS_MAP: Record<CiStatus, { label: string; color: string }> = {
  success: { label: "PASS", color: "text-green" },
  failure: { label: "FAIL", color: "text-red" },
  running: { label: "RUN", color: "text-orange" },
  pending: { label: "PEND", color: "text-dim" },
  cancelled: { label: "CANCEL", color: "text-dim" },
};

export function CI({ status }: CIProps): ReactElement {
  const { label, color } = STATUS_MAP[status];
  return (
    <span className={`text-xs font-semibold uppercase ${color}`}>{label}</span>
  );
}
