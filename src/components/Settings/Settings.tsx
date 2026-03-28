import { useEffect, useState, type ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getConfig,
  setConfig,
  listRepos,
  setRepoEnabled,
  authGetStatus,
} from "../../lib/tauri";
import type { PartialAppConfig, Repo } from "../../lib/types";

function useConfigQuery() {
  return useQuery({ queryKey: ["config"], queryFn: getConfig });
}

function useReposQuery() {
  return useQuery({ queryKey: ["repos"], queryFn: listRepos });
}

function useAuthQuery() {
  return useQuery({ queryKey: ["auth", "status"], queryFn: authGetStatus });
}

interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  readonly min?: number;
  readonly onCommit: (value: number) => void;
}

function NumberField({ label, value, min = 1, onCommit }: NumberFieldProps): ReactElement {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  function handleBlur(): void {
    const parsed = Number(draft);
    if (!Number.isNaN(parsed) && parsed >= min && parsed !== value) {
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
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={handleBlur}
        className="bg-surface border-border w-24 rounded border px-2 py-1 font-mono text-sm text-white"
      />
    </label>
  );
}

interface RepoRowProps {
  readonly repo: Repo;
  readonly onToggle: (repoId: string, enabled: boolean) => void;
}

function RepoRow({ repo, onToggle }: RepoRowProps): ReactElement {
  return (
    <label className="flex items-center justify-between gap-3 py-1">
      <span className="min-w-0 truncate font-mono text-sm text-white">{repo.name}</span>
      <input
        type="checkbox"
        checked={repo.enabled}
        onChange={() => onToggle(repo.id, !repo.enabled)}
        className="accent-accent h-4 w-4"
      />
    </label>
  );
}

export function Settings(): ReactElement {
  const queryClient = useQueryClient();
  const configQuery = useConfigQuery();
  const reposQuery = useReposQuery();
  const authQuery = useAuthQuery();

  const configMutation = useMutation({
    mutationFn: (partial: PartialAppConfig) => setConfig(partial),
    onSuccess: (updated) => {
      queryClient.setQueryData(["config"], updated);
    },
    onError: (err: unknown) => {
      console.error("[Settings] config update failed:", err);
    },
  });

  const repoToggleMutation = useMutation({
    mutationFn: ({ repoId, enabled }: { repoId: string; enabled: boolean }) =>
      setRepoEnabled(repoId, enabled),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
    onError: (err: unknown) => {
      console.error("[Settings] repo toggle failed:", err);
    },
  });

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
  const repos = reposQuery.data ?? [];
  const auth = authQuery.data;

  return (
    <section data-testid="settings" className="flex h-full flex-col gap-6 overflow-y-auto p-4">
      <h1 className="text-lg font-semibold text-white">Settings</h1>

      <div data-testid="settings-github" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">GitHub</h2>
        <NumberField
          label="Poll interval (seconds)"
          value={config.pollIntervalSecs}
          onCommit={(v) => configMutation.mutate({ pollIntervalSecs: v })}
        />
        <div className="flex items-center justify-between text-sm">
          <span className="text-dim">Auth status</span>
          {auth?.connected ? (
            <span className="text-green-400">Connected — {auth.username}</span>
          ) : (
            <span className="text-red-400">Not connected</span>
          )}
        </div>
      </div>

      <div data-testid="settings-workspaces" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Workspaces</h2>
        <NumberField
          label="Max active workspaces"
          value={config.maxActiveWorkspaces}
          onCommit={(v) => configMutation.mutate({ maxActiveWorkspaces: v })}
        />
      </div>

      <div data-testid="settings-repos" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Repositories</h2>
        {repos.length === 0 ? (
          <span className="text-dim text-sm">No repositories synced</span>
        ) : (
          <div className="flex flex-col gap-1">
            {repos.map((repo) => (
              <RepoRow
                key={repo.id}
                repo={repo}
                onToggle={(repoId, enabled) =>
                  repoToggleMutation.mutate({ repoId, enabled })
                }
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}
