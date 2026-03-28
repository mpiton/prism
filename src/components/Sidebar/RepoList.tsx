import type { ReactElement } from "react";
import type { Repo } from "../../lib/types";

interface RepoListProps {
  readonly repos: readonly Repo[];
  readonly onToggleRepo: (repoId: string, enabled: boolean) => void;
}

export function RepoList({
  repos,
  onToggleRepo,
}: RepoListProps): ReactElement {
  return (
    <div className="flex flex-col gap-0.5">
      {repos.map((repo) => (
        <label
          key={repo.id}
          className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-xs text-dim hover:bg-surface-hover hover:text-foreground"
        >
          <input
            type="checkbox"
            checked={repo.enabled}
            onChange={() => onToggleRepo(repo.id, !repo.enabled)}
            aria-label={`Enable ${repo.fullName} repository`}
            className="accent-accent"
          />
          <span>{repo.fullName}</span>
        </label>
      ))}
    </div>
  );
}
