import { useEffect, useState, type ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getConfig,
  setConfig,
  listRepos,
  setRepoEnabled,
} from "../../lib/tauri";
import type { PartialAppConfig, Repo } from "../../lib/types";
import { AuthSetup } from "../AuthSetup/AuthSetup";
import { Stats } from "./Stats";
import { DebugInfo } from "./DebugInfo";
import { useDebounce } from "../../hooks/useDebounce";

function useConfigQuery() {
  return useQuery({ queryKey: ["config"], queryFn: getConfig });
}

function useReposQuery() {
  return useQuery({ queryKey: ["repos"], queryFn: listRepos });
}


interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  readonly min?: number;
  readonly resetKey: number;
  readonly onCommit: (value: number) => void;
}

function NumberField({ label, value, min = 1, resetKey, onCommit }: NumberFieldProps): ReactElement {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value, resetKey]);

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
        className="bg-surface border-border w-24 rounded border px-2 py-1 font-mono text-sm text-white"
      />
    </label>
  );
}

interface RepoRowProps {
  readonly repo: Repo;
  readonly disabled: boolean;
  readonly onToggle: (repoId: string, enabled: boolean) => void;
}

function RepoRow({ repo, disabled, onToggle }: RepoRowProps): ReactElement {
  return (
    <label className="flex items-center justify-between gap-3 py-1">
      <span className="min-w-0 truncate font-mono text-sm text-white" title={repo.name}>{repo.name}</span>
      <input
        type="checkbox"
        checked={repo.enabled}
        disabled={disabled}
        onChange={() => onToggle(repo.id, !repo.enabled)}
        className="accent-accent h-4 w-4"
      />
    </label>
  );
}

const sectionClass = "flex flex-col gap-3 border-b border-border pb-4";

export function Settings(): ReactElement {
  const queryClient = useQueryClient();
  const configQuery = useConfigQuery();
  const reposQuery = useReposQuery();
  const [saveError, setSaveError] = useState<string | null>(null);
  const [resetKey, setResetKey] = useState(0);
  const [repoSearch, setRepoSearch] = useState("");
  const [showAllRepos, setShowAllRepos] = useState(false);
  const debouncedRepoSearch = useDebounce(repoSearch, 150);

  useEffect(() => {
    setShowAllRepos(false);
  }, [debouncedRepoSearch]);

  const configMutation = useMutation({
    mutationFn: (partial: PartialAppConfig) => setConfig(partial),
    onMutate: () => {
      setSaveError(null);
    },
    onSuccess: (updated) => {
      queryClient.setQueryData(["config"], updated);
    },
    onError: (err: unknown) => {
      console.error("[Settings] config update failed:", err);
      setSaveError("Failed to save setting. Please retry.");
      setResetKey((k) => k + 1);
    },
  });

  const repoToggleMutation = useMutation({
    mutationFn: ({ repoId, enabled }: { repoId: string; enabled: boolean }) =>
      setRepoEnabled(repoId, enabled),
    onSuccess: () => {
      setSaveError(null);
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
    onError: (err: unknown) => {
      console.error("[Settings] repo toggle failed:", err);
      setSaveError("Failed to toggle repository. Please retry.");
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
  const allRepos = reposQuery.data ?? [];
  const searchLower = debouncedRepoSearch.toLowerCase();
  const filteredSettingsRepos = allRepos.filter(
    (r) =>
      r.name.toLowerCase().includes(searchLower) ||
      r.fullName.toLowerCase().includes(searchLower),
  );
  const visibleSettingsRepos = showAllRepos
    ? filteredSettingsRepos
    : filteredSettingsRepos.slice(0, 10);

  return (
    <section data-testid="settings" className="flex h-full flex-col gap-6 overflow-y-auto p-4">
      <h1 className="text-lg font-semibold text-white">Settings</h1>

      {saveError ? (
        <p role="alert" className="text-sm text-red-400">{saveError}</p>
      ) : null}

      <div data-testid="settings-github" className={sectionClass}>
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">GitHub</h2>
        <AuthSetup />
        <NumberField
          label="Poll interval (seconds)"
          value={config.pollIntervalSecs}
          resetKey={resetKey}
          onCommit={(v) => configMutation.mutate({ pollIntervalSecs: v })}
        />
      </div>

      <div data-testid="settings-workspaces" className={sectionClass}>
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Workspaces</h2>
        <NumberField
          label="Max active workspaces"
          value={config.maxActiveWorkspaces}
          resetKey={resetKey}
          onCommit={(v) => configMutation.mutate({ maxActiveWorkspaces: v })}
        />
      </div>

      <div data-testid="settings-repos" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Repositories</h2>
        <input
          type="search"
          placeholder="Filter repositories..."
          value={repoSearch}
          onChange={(e) => setRepoSearch(e.target.value)}
          className="bg-surface border-border rounded border px-2 py-1 text-sm text-white placeholder:text-muted outline-none"
        />
        {reposQuery.isLoading ? (
          <span className="text-dim text-sm">Loading repositories...</span>
        ) : reposQuery.error ? (
          <span className="text-sm text-red-400">Failed to load repositories</span>
        ) : allRepos.length === 0 ? (
          <span className="text-dim text-sm">No repositories synced</span>
        ) : filteredSettingsRepos.length === 0 ? (
          <span className="text-dim text-sm">No repos match</span>
        ) : (
          <div className="flex flex-col gap-1">
            {visibleSettingsRepos.map((repo) => (
              <RepoRow
                key={repo.id}
                repo={repo}
                disabled={repoToggleMutation.isPending}
                onToggle={(repoId, enabled) =>
                  repoToggleMutation.mutate({ repoId, enabled })
                }
              />
            ))}
            {!showAllRepos && filteredSettingsRepos.length > 10 && (
              <button
                type="button"
                onClick={() => setShowAllRepos(true)}
                className="text-xs text-muted hover:text-foreground transition-colors"
              >
                Show {filteredSettingsRepos.length - 10} more
              </button>
            )}
          </div>
        )}
      </div>

      <Stats />

      <DebugInfo />
    </section>
  );
}
