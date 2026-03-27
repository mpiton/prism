import type { ReactElement } from "react";

interface SectionHeadProps {
  readonly title: string;
  readonly count: number;
}

export function SectionHead({ title, count }: SectionHeadProps): ReactElement {
  return (
    <div>
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-white">{title}</h2>
        <span className="text-xs text-dim">{count}</span>
      </div>
      <hr className="mt-1 border-border" role="separator" />
    </div>
  );
}
