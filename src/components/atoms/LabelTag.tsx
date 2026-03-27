import type { ReactElement } from "react";

interface LabelTagProps {
  readonly name: string;
}

function colorForLabel(name: string): { bg: string; text: string } {
  const lower = name.toLowerCase();
  if (lower.includes("bug") || lower.includes("error")) {
    return { bg: "bg-red/20", text: "text-red" };
  }
  if (lower.includes("feature") || lower.includes("enhancement")) {
    return { bg: "bg-green/20", text: "text-green" };
  }
  if (lower.includes("doc")) {
    return { bg: "bg-blue/20", text: "text-blue" };
  }
  if (lower.includes("fix")) {
    return { bg: "bg-orange/20", text: "text-orange" };
  }
  return { bg: "bg-purple/20", text: "text-purple" };
}

export function LabelTag({ name }: LabelTagProps): ReactElement {
  const { bg, text } = colorForLabel(name);
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-xs ${bg} ${text}`}>
      {name}
    </span>
  );
}
