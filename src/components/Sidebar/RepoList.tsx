import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import type { Repo } from "../../lib/types";
import { useDebounce } from "../../hooks/useDebounce";

const VISIBLE_LIMIT = 6;

interface RepoListProps {
  readonly repos: readonly Repo[];
  readonly onToggleRepo: (repoId: string, enabled: boolean) => void;
  readonly onSelectAll?: () => void;
  readonly onDeselectAll?: () => void;
}

export function RepoList({
  repos,
  onToggleRepo,
  onSelectAll,
  onDeselectAll,
}: RepoListProps): ReactElement {
  const [searchQuery, setSearchQuery] = useState("");
  const [showAll, setShowAll] = useState(false);
  const debouncedQuery = useDebounce(searchQuery, 150);

  useEffect(() => {
    setShowAll(false);
  }, [debouncedQuery]);

  const filteredRepos = repos.filter((repo) =>
    repo.fullName.toLowerCase().includes(debouncedQuery.toLowerCase()),
  );

  const visibleRepos =
    filteredRepos.length > VISIBLE_LIMIT && !showAll
      ? filteredRepos.slice(0, VISIBLE_LIMIT)
      : filteredRepos;

  const hiddenCount = filteredRepos.length - VISIBLE_LIMIT;

  return (
    <div className="flex flex-col gap-0.5">
      <div className="flex items-center justify-between px-1">
        <input
          type="search"
          placeholder="Filter repos..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full bg-transparent border-b border-border text-xs text-foreground placeholder:text-muted px-2 py-1 outline-none"
        />
        {(onSelectAll !== undefined || onDeselectAll !== undefined) && (
          <div className="flex shrink-0 gap-1 pl-1">
            {onSelectAll !== undefined && (
              <button
                type="button"
                onClick={onSelectAll}
                className="text-[10px] text-dim hover:text-foreground"
              >
                Select all
              </button>
            )}
            {onDeselectAll !== undefined && (
              <button
                type="button"
                onClick={onDeselectAll}
                className="text-[10px] text-dim hover:text-foreground"
              >
                Deselect all
              </button>
            )}
          </div>
        )}
      </div>

      {filteredRepos.length === 0 && debouncedQuery.length > 0 ? (
        <p className="px-2 py-2 text-xs text-muted">No repos match</p>
      ) : (
        <>
          {visibleRepos.map((repo) => (
            <label
              key={repo.id}
              className="flex cursor-pointer items-center gap-2 rounded px-2 py-2 text-xs text-dim hover:bg-surface-hover hover:text-foreground"
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

          {filteredRepos.length > VISIBLE_LIMIT && !showAll && (
            <button
              type="button"
              onClick={() => setShowAll(true)}
              className="px-2 py-1 text-left text-xs text-dim hover:text-foreground"
            >
              Show {hiddenCount} more
            </button>
          )}
        </>
      )}
    </div>
  );
}
