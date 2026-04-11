import { useState, type ReactElement } from "react";
import type { AppConfig } from "../../lib/types/config";
import { NumberField } from "./NumberField";
import { useConfigMutation } from "./useConfigMutation";
import { sectionClass } from "./styles";

interface WorkspaceSettingsProps {
  readonly config: AppConfig;
  readonly onError: (message: string | null) => void;
}

export function WorkspaceSettings({ config, onError }: WorkspaceSettingsProps): ReactElement {
  const [resetKey, setResetKey] = useState(0);
  const configMutation = useConfigMutation({
    onError,
    onResetDraft: () => setResetKey((k) => k + 1),
  });

  return (
    <div data-testid="settings-workspaces" className={sectionClass}>
      <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Workspaces</h2>
      <NumberField
        key={resetKey}
        label="Max active workspaces"
        value={config.maxActiveWorkspaces}
        onCommit={(v) => configMutation.mutate({ maxActiveWorkspaces: v })}
      />
    </div>
  );
}
