import type { ReactElement } from "react";
import type { DashboardView } from "../../stores/dashboard";

interface NavItemProps {
  readonly label: string;
  readonly view: DashboardView;
  readonly count?: number;
  readonly isActive: boolean;
  readonly onClick: (view: DashboardView) => void;
}

export function NavItem({
  label,
  view,
  count,
  isActive,
  onClick,
}: NavItemProps): ReactElement {
  return (
    <button
      type="button"
      onClick={() => onClick(view)}
      aria-current={isActive ? "page" : undefined}
      aria-label={count !== undefined && count > 0 ? `${label} (${count})` : undefined}
      className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm ${
        isActive
          ? "bg-surface text-white"
          : "text-dim hover:bg-surface-hover hover:text-foreground"
      }`}
    >
      <span>{label}</span>
      {count !== undefined && count > 0 ? (
        <span className="min-w-[1.25rem] rounded-full bg-accent/15 px-1.5 text-center text-xs text-accent">
          {count}
        </span>
      ) : null}
    </button>
  );
}
