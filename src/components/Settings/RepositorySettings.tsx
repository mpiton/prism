import { useState, type ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { FOCUS_RING } from "../../lib/a11y";
import { listRepos, setRepoEnabled } from "../../lib/tauri";
import type { Repo } from "../../lib/types/github";
import { useDebounce } from "../../hooks/useDebounce";

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

const controlButtonClass = `${FOCUS_RING} rounded border border-border px-2 py-1 text-xs font-medium text-dim transition-colors hover:border-accent hover:text-white disabled:cursor-not-allowed disabled:opacity-50`;

interface RepositorySettingsProps {
  readonly onError: (message: string | null) => void;
}

export function RepositorySettings({
  onError: reportError,
}: RepositorySettingsProps): ReactElement {
  const queryClient = useQueryClient();
  const reposQuery = useQuery({ queryKey: ["repos"], queryFn: listRepos });
  const [repoSearch, setRepoSearch] = useState("");
  const debouncedRepoSearch = useDebounce(repoSearch, 150);

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
    onMutate: () => reportError(null),
    onError: (err: unknown) => {
      console.error("[RepositorySettings] repo toggle failed:", err);
      reportError(getRepoBatchUpdateErrorMessage(err));
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
  });

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
    <div data-testid="settings-repos" className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Repositories</h2>
        <p className="text-dim text-sm">
          {enabledReposCount} of {allRepos.length} repositories enabled
        </p>
      </div>
      <label className="flex flex-col gap-1">
        <span className="sr-only">Filter repositories</span>
        <input
          type="search"
          placeholder="Filter repositories..."
          value={repoSearch}
          onChange={(e) => setRepoSearch(e.target.value)}
          className={`${FOCUS_RING} bg-surface border-border rounded border px-2 py-1 text-sm text-white placeholder:text-muted`}
        />
      </label>
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
                <h3 className="font-mono text-xs uppercase tracking-wide text-dim">{group.org}/</h3>
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
  );
}
