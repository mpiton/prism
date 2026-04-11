import { useEffect, useState, type ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { FOCUS_RING } from "../../lib/a11y";
import { getConfig, setConfig, listRepos, setRepoEnabled } from "../../lib/tauri";
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

interface RepoUpdate {
  readonly repoId: string;
  readonly enabled: boolean;
}

interface RepoGroup {
  readonly org: string;
  readonly repos: readonly Repo[];
}

interface RepoBatchUpdateError extends Error {
  failedRepoIds: readonly string[];
}

interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  readonly min?: number;
  readonly resetKey: number;
  readonly onCommit: (value: number) => void;
}

function NumberField({
  label,
  value,
  min = 1,
  resetKey,
  onCommit,
}: NumberFieldProps): ReactElement {
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
        className={`${FOCUS_RING} bg-surface border-border w-24 rounded border px-2 py-1 font-mono text-sm text-white`}
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
      <span className="min-w-0 truncate font-mono text-sm text-white" title={repo.name}>
        {repo.name}
      </span>
      <input
        type="checkbox"
        checked={repo.enabled}
        disabled={disabled}
        onChange={() => onToggle(repo.id, !repo.enabled)}
        className={`${FOCUS_RING} accent-accent h-4 w-4`}
      />
    </label>
  );
}

function groupReposByOrg(repos: readonly Repo[]): readonly RepoGroup[] {
  const groups = new Map<string, Repo[]>();

  for (const repo of repos) {
    const existing = groups.get(repo.org);
    if (existing) {
      existing.push(repo);
    } else {
      groups.set(repo.org, [repo]);
    }
  }

  return Array.from(groups.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([org, orgRepos]) => ({
      org,
      repos: [...orgRepos].sort((left, right) => left.name.localeCompare(right.name)),
    }));
}

function buildRepoUpdates(
  repos: readonly Repo[],
  getNextEnabled: (repo: Repo) => boolean,
): readonly RepoUpdate[] {
  const updates: RepoUpdate[] = [];

  for (const repo of repos) {
    const enabled = getNextEnabled(repo);
    if (enabled !== repo.enabled) {
      updates.push({ repoId: repo.id, enabled });
    }
  }

  return updates;
}

function createRepoBatchUpdateError(failedRepoIds: readonly string[]): RepoBatchUpdateError {
  const error = new Error("Failed to update some repositories.") as RepoBatchUpdateError;
  error.failedRepoIds = failedRepoIds;
  return error;
}

function getRepoBatchUpdateErrorMessage(err: unknown): string {
  if (
    typeof err === "object" &&
    err !== null &&
    "failedRepoIds" in err &&
    Array.isArray(err.failedRepoIds) &&
    err.failedRepoIds.length > 0
  ) {
    return `Failed to update repositories: ${err.failedRepoIds.join(", ")}`;
  }

  return "Failed to update repositories. Please retry.";
}

const sectionClass = "flex flex-col gap-3 border-b border-border pb-4";
const controlButtonClass = `${FOCUS_RING} rounded border border-border px-2 py-1 text-xs font-medium text-dim transition-colors hover:border-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-50`;

export function Settings(): ReactElement {
  const queryClient = useQueryClient();
  const configQuery = useConfigQuery();
  const reposQuery = useReposQuery();
  const [saveError, setSaveError] = useState<string | null>(null);
  const [resetKey, setResetKey] = useState(0);
  const [repoSearch, setRepoSearch] = useState("");
  const debouncedRepoSearch = useDebounce(repoSearch, 150);

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
    mutationFn: async (updates: readonly RepoUpdate[]) => {
      const results = await Promise.allSettled(
        updates.map(({ repoId, enabled }) => setRepoEnabled(repoId, enabled)),
      );
      const failedRepoIds: string[] = [];

      for (const [index, result] of results.entries()) {
        const update = updates[index];
        if (result.status === "rejected" && update) {
          failedRepoIds.push(update.repoId);
        }
      }

      if (failedRepoIds.length > 0) {
        throw createRepoBatchUpdateError(failedRepoIds);
      }
    },
    onSuccess: () => {
      setSaveError(null);
    },
    onError: (err: unknown) => {
      console.error("[Settings] repo toggle failed:", err);
      setSaveError(getRepoBatchUpdateErrorMessage(err));
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["repos"] });
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
      r.name.toLowerCase().includes(searchLower) || r.fullName.toLowerCase().includes(searchLower),
  );
  const groupedSettingsRepos = groupReposByOrg(filteredSettingsRepos);
  const enabledReposCount = allRepos.filter((repo) => repo.enabled).length;

  function mutateRepoUpdates(updates: readonly RepoUpdate[]): void {
    if (updates.length === 0) {
      return;
    }

    repoToggleMutation.mutate(updates);
  }

  return (
    <section data-testid="settings" className="flex h-full flex-col gap-6 overflow-y-auto p-4">
      <h1 className="text-lg font-semibold text-white">Settings</h1>

      {saveError ? (
        <p role="alert" className="text-sm text-red-400">
          {saveError}
        </p>
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
        <div className="flex flex-col gap-1">
          <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">
            Repositories
          </h2>
          <p className="text-dim text-sm">
            {enabledReposCount} of {allRepos.length} repositories enabled
          </p>
        </div>
        <input
          type="search"
          placeholder="Filter repositories..."
          value={repoSearch}
          onChange={(e) => setRepoSearch(e.target.value)}
          className={`${FOCUS_RING} bg-surface border-border rounded border px-2 py-1 text-sm text-white placeholder:text-muted`}
        />
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => mutateRepoUpdates(buildRepoUpdates(filteredSettingsRepos, () => true))}
            disabled={repoToggleMutation.isPending || filteredSettingsRepos.length === 0}
            className={controlButtonClass}
          >
            Select all
          </button>
          <button
            type="button"
            onClick={() => mutateRepoUpdates(buildRepoUpdates(filteredSettingsRepos, () => false))}
            disabled={repoToggleMutation.isPending || filteredSettingsRepos.length === 0}
            className={controlButtonClass}
          >
            Deselect all
          </button>
          <button
            type="button"
            onClick={() =>
              mutateRepoUpdates(buildRepoUpdates(filteredSettingsRepos, (repo) => !repo.enabled))
            }
            disabled={repoToggleMutation.isPending || filteredSettingsRepos.length === 0}
            className={controlButtonClass}
          >
            Invert selection
          </button>
          {debouncedRepoSearch.trim() ? (
            <span className="text-dim text-xs">
              {filteredSettingsRepos.length} matching current filter
            </span>
          ) : null}
        </div>
        {reposQuery.isLoading ? (
          <span className="text-dim text-sm">Loading repositories...</span>
        ) : reposQuery.error ? (
          <span className="text-sm text-red-400">Failed to load repositories</span>
        ) : allRepos.length === 0 ? (
          <span className="text-dim text-sm">No repositories synced</span>
        ) : filteredSettingsRepos.length === 0 ? (
          <span className="text-dim text-sm">No repos match</span>
        ) : (
          <div className="flex flex-col gap-4">
            {groupedSettingsRepos.map((group) => (
              <div key={group.org} className="flex flex-col gap-1">
                <div className="flex items-center justify-between gap-3">
                  <h3 className="font-mono text-xs uppercase tracking-wide text-dim">
                    {group.org}/
                  </h3>
                  <span className="text-dim text-xs">
                    {group.repos.filter((repo) => repo.enabled).length}/{group.repos.length} enabled
                  </span>
                </div>
                {group.repos.map((repo) => (
                  <RepoRow
                    key={repo.id}
                    repo={repo}
                    disabled={repoToggleMutation.isPending}
                    onToggle={(repoId, enabled) => mutateRepoUpdates([{ repoId, enabled }])}
                  />
                ))}
              </div>
            ))}
          </div>
        )}
      </div>

      <Stats />

      <DebugInfo />
    </section>
  );
}
