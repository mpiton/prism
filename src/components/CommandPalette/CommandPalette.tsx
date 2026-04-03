import { useCallback, useMemo, useState } from "react";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { useGitHubData } from "../../hooks/useGitHubData";
import { listRepos } from "../../lib/tauri";
import { useDashboardStore } from "../../stores/dashboard";
import type { DashboardView } from "../../stores/dashboard";
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
  readonly repoName: string;
  readonly section: DashboardView;
}

function prToItem(pr: PullRequestWithReview, repoMap: Map<string, string>): PaletteItem {
  return {
    id: pr.pullRequest.id,
    type: "pr",
    number: pr.pullRequest.number,
    title: pr.pullRequest.title,
    url: pr.pullRequest.url,
    repoName: repoMap.get(pr.pullRequest.repoId) ?? pr.pullRequest.repoId,
    section: "reviews",
  };
}

function issueToItem(issue: Issue, repoMap: Map<string, string>): PaletteItem {
  return {
    id: issue.id,
    type: "issue",
    number: issue.number,
    title: issue.title,
    url: issue.url,
    repoName: repoMap.get(issue.repoId) ?? issue.repoId,
    section: "issues",
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
  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });
  const [selectedValue, setSelectedValue] = useState("");

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();
    return new Map(repos.map((r) => [r.id, r.name]));
  }, [repos]);

  const { prItems, issueItems } = useMemo(() => {
    if (!dashboard) return { prItems: [], issueItems: [] };

    const allPrItems = deduplicateItems([
      ...dashboard.reviewRequests.map((pr) => prToItem(pr, repoMap)),
      ...dashboard.myPullRequests.map((pr) => prToItem(pr, repoMap)),
    ]);
    const allIssueItems = dashboard.assignedIssues.map((issue) =>
      issueToItem(issue, repoMap),
    );

    return { prItems: allPrItems, issueItems: allIssueItems };
  }, [dashboard, repoMap]);

  const allItems = useMemo(
    () => [...prItems, ...issueItems],
    [prItems, issueItems],
  );

  const findSelectedItem = useCallback(
    () => allItems.find((item) => `${item.number} ${item.title}` === selectedValue) ?? null,
    [allItems, selectedValue],
  );

  function handleSelect(item: PaletteItem): void {
    useDashboardStore.getState().setView(item.section);
    onOpenChange(false);
  }

  function handleKeyDown(e: React.KeyboardEvent): void {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      const item = findSelectedItem();
      if (item) {
        window.open(item.url, "_blank", "noopener,noreferrer");
        onOpenChange(false);
      }
    }
  }

  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      label="Command palette"
      aria-describedby={undefined}
      className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]"
      onKeyDown={handleKeyDown}
      value={selectedValue}
      onValueChange={setSelectedValue}
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

          {prItems.length > 0 && (
            <Command.Group heading="Pull Requests">
              {prItems.map((item) => (
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
                  <span className="shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs text-fg/60">
                    {item.repoName}
                  </span>
                  <span className="ml-auto shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs uppercase text-fg/60">
                    {item.type}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          )}

          {issueItems.length > 0 && (
            <Command.Group heading="Issues">
              {issueItems.map((item) => (
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
                  <span className="shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs text-fg/60">
                    {item.repoName}
                  </span>
                  <span className="ml-auto shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs uppercase text-fg/60">
                    {item.type}
                  </span>
                </Command.Item>
              ))}
            </Command.Group>
          )}
        </Command.List>
      </div>
    </Command.Dialog>
  );
}
