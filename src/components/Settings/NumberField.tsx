import { useState, type ReactElement } from "react";
import { FOCUS_RING } from "../../lib/a11y";

interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  readonly min?: number;
  readonly onCommit: (value: number) => void;
}

export function NumberField({ label, value, min = 1, onCommit }: NumberFieldProps): ReactElement {
  const [draft, setDraft] = useState(String(value));

  function handleBlur(): void {
    const parsed = Number(draft);
    if (Number.isInteger(parsed) && parsed >= min && parsed !== value) {
      onCommit(parsed);
    } else {
      setDraft(String(value));
    }
  }

  return (
    <label className="flex items-center justify-between gap-4">
      <span className="text-dim text-sm">{label}</span>
      <input
        type="number"
        min={min}
        step={1}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={handleBlur}
        className={`${FOCUS_RING} bg-surface border-border w-24 rounded border px-2 py-1 font-mono text-sm text-white`}
      />
    </label>
  );
}
