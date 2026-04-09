import type { ReactElement } from "react";

interface SectionHeadProps {
  readonly title: string;
  readonly count?: number;
}

export function SectionHead({ title, count }: SectionHeadProps): ReactElement {
  const accessibleTitle = typeof count === "number" ? `${title} ${count}` : title;

  return (
    <div>
      <h2 className="text-sm font-semibold text-white" aria-label={accessibleTitle}>
        {title}
        {typeof count === "number" && <span className="text-xs font-normal text-dim"> {count}</span>}
      </h2>
      <hr className="mt-1 border-border" role="separator" />
    </div>
  );
}
