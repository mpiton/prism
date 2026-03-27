import type { ReactElement } from "react";

interface DiffProps {
  readonly additions: number;
  readonly deletions: number;
}

export function Diff({ additions, deletions }: DiffProps): ReactElement {
  return (
    <span className="font-mono text-xs">
      <span className="text-green">+{additions}</span>
      {" "}
      <span className="text-red">-{deletions}</span>
    </span>
  );
}
