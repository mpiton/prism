import { useState, type ReactElement } from "react";
import { useQuery } from "@tanstack/react-query";
import { getConfig } from "../../lib/tauri";
import { GitHubSettings } from "./GitHubSettings";
import { WorkspaceSettings } from "./WorkspaceSettings";
import { RepositorySettings } from "./RepositorySettings";
import { Stats } from "./Stats";
import { DebugInfo } from "./DebugInfo";

function useConfigQuery() {
  return useQuery({ queryKey: ["config"], queryFn: getConfig });
}

export function Settings(): ReactElement {
  const configQuery = useConfigQuery();
  const [saveError, setSaveError] = useState<string | null>(null);

  if (configQuery.error && !configQuery.data) {
    return (
      <div data-testid="settings" className="flex h-full items-center justify-center text-dim">
        Failed to load settings
      </div>
    );
  }

  if (!configQuery.data) {
    return (
      <div data-testid="settings" className="flex h-full items-center justify-center text-dim">
        Loading...
      </div>
    );
  }

  const config = configQuery.data;

  return (
    <section data-testid="settings" className="flex h-full flex-col gap-6 overflow-y-auto p-4">
      <h1 className="text-lg font-semibold text-white">Settings</h1>

      {saveError ? (
        <p role="alert" className="text-sm text-red-400">
          {saveError}
        </p>
      ) : null}

      <GitHubSettings config={config} onError={setSaveError} />
      <WorkspaceSettings config={config} onError={setSaveError} />
      <RepositorySettings onError={setSaveError} />

      <Stats />

      <DebugInfo />
    </section>
  );
}
