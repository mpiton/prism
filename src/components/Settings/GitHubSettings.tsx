import { useState, type ReactElement } from "react";
import type { AppConfig } from "../../lib/types/config";
import { AuthSetup } from "../AuthSetup";
import { NumberField } from "./NumberField";
import { useConfigMutation } from "./useConfigMutation";
import { sectionClass } from "./styles";

interface GitHubSettingsProps {
  readonly config: AppConfig;
  readonly onError: (message: string | null) => void;
}

export function GitHubSettings({ config, onError }: GitHubSettingsProps): ReactElement {
  const [resetKey, setResetKey] = useState(0);
  const configMutation = useConfigMutation({
    onError,
    onResetDraft: () => setResetKey((k) => k + 1),
  });

  return (
    <div data-testid="settings-github" className={sectionClass}>
      <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">GitHub</h2>
      <AuthSetup />
      <NumberField
        key={resetKey}
        label="Poll interval (seconds)"
        value={config.pollIntervalSecs}
        onCommit={(v) => configMutation.mutate({ pollIntervalSecs: v })}
      />
    </div>
  );
}
