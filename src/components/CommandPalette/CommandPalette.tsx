import { useMemo } from "react";
import { Command } from "cmdk";
import { useGitHubData } from "../../hooks/useGitHubData";
import type { PullRequestWithReview, Issue } from "../../lib/types";

interface CommandPaletteProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
}

interface PaletteItem {
  readonly id: string;
  readonly type: "pr" | "issue";
  readonly number: number;
  readonly title: string;
  readonly url: string;
}

function prToItem(pr: PullRequestWithReview): PaletteItem {
  return {
    id: pr.pullRequest.id,
    type: "pr",
    number: pr.pullRequest.number,
    title: pr.pullRequest.title,
    url: pr.pullRequest.url,
  };
}

function issueToItem(issue: Issue): PaletteItem {
  return {
    id: issue.id,
    type: "issue",
    number: issue.number,
    title: issue.title,
    url: issue.url,
  };
}

function deduplicateItems(items: readonly PaletteItem[]): readonly PaletteItem[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    if (seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  });
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const { dashboard } = useGitHubData();

  const items = useMemo<readonly PaletteItem[]>(() => {
    if (!dashboard) return [];

    const prItems = [
      ...dashboard.reviewRequests.map(prToItem),
      ...dashboard.myPullRequests.map(prToItem),
    ];
    const issueItems = dashboard.assignedIssues.map(issueToItem);

    return deduplicateItems([...prItems, ...issueItems]);
  }, [dashboard]);

  function handleSelect(item: PaletteItem): void {
    window.open(item.url, "_blank", "noopener,noreferrer");
    onOpenChange(false);
  }

  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      label="Command palette"
      className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
    >
      <div className="w-full max-w-lg rounded-lg border border-border bg-bg shadow-xl">
        <Command.Input
          placeholder="Search PRs and issues…"
          className="w-full border-b border-border bg-transparent px-4 py-3 text-sm text-fg outline-none placeholder:text-fg/50"
        />
        <Command.List className="max-h-80 overflow-y-auto p-2">
          <Command.Empty className="px-4 py-6 text-center text-sm text-fg/50">
            No results found.
          </Command.Empty>

          {items.map((item) => (
            <Command.Item
              key={item.id}
              value={`${item.number} ${item.title}`}
              onSelect={() => handleSelect(item)}
              className="flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-fg aria-selected:bg-fg/10"
            >
              <span className="shrink-0 text-fg/50">
                #{item.number}
              </span>
              <span className="truncate">{item.title}</span>
              <span className="ml-auto shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs uppercase text-fg/60">
                {item.type}
              </span>
            </Command.Item>
          ))}
        </Command.List>
      </div>
    </Command.Dialog>
  );
}
