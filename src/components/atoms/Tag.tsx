import type { ReactElement, ReactNode } from "react";

interface LabelColor {
  readonly bg: string;
  readonly text: string;
}

const RED: LabelColor = { bg: "bg-red/20", text: "text-red" };
const GREEN: LabelColor = { bg: "bg-green/20", text: "text-green" };
const BLUE: LabelColor = { bg: "bg-blue/20", text: "text-blue" };
const ORANGE: LabelColor = { bg: "bg-orange/20", text: "text-orange" };
const PURPLE: LabelColor = { bg: "bg-purple/20", text: "text-purple" };

const LABEL_COLORS: Record<string, LabelColor> = {
  bug: RED,
  error: RED,
  feature: GREEN,
  enhancement: GREEN,
  documentation: BLUE,
  docs: BLUE,
  fix: ORANGE,
  hotfix: ORANGE,
};

function colorForLabel(name: string): LabelColor {
  return LABEL_COLORS[name.toLowerCase()] ?? PURPLE;
}

interface TagProps {
  readonly children?: ReactNode;
  readonly className?: string;
  readonly variant?: "default" | "label";
  readonly label?: string;
}

export function Tag({ children, className = "", variant = "default", label }: TagProps): ReactElement {
  if (variant === "label" && label !== undefined) {
    const { bg, text } = colorForLabel(label);
    return (
      <span className={`inline-block rounded-full px-2 py-0.5 text-xs ${bg} ${text}`}>
        {label}
      </span>
    );
  }

  return (
    <span className={`text-xs font-medium uppercase tracking-wide ${className || "text-dim"}`}>
      {children}
    </span>
  );
}
