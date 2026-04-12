import { useCallback, useEffect, useMemo, useState, type ReactElement } from "react";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { useGitHubData } from "../../hooks/useGitHubData";
import { FOCUS_RING } from "../../lib/a11y";
import {
  BookOpen,
  Download,
  Focus,
  FolderOpen,
  LayoutDashboard,
  RefreshCw,
  Settings as SettingsIcon,
} from "../../lib/icons";
import { listRepos } from "../../lib/tauri";
import { useDashboardStore } from "../../stores/dashboard";
import { openUrl } from "../../lib/open";
import type { DashboardView } from "../../stores/dashboard";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import type { Issue } from "../../lib/types/github";

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

function prToItem(
  pr: PullRequestWithReview,
  repoMap: Map<string, string>,
  section: DashboardView,
): PaletteItem {
  return {
    id: pr.pullRequest.id,
    type: "pr",
    number: pr.pullRequest.number,
    title: pr.pullRequest.title,
    url: pr.pullRequest.url,
    repoName: repoMap.get(pr.pullRequest.repoId) ?? pr.pullRequest.repoId,
    section,
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

const ITEM_CLASS =
  "flex cursor-pointer items-center gap-2 rounded-md px-3 py-2 text-sm text-fg aria-selected:bg-fg/10";

function PaletteItemRow({ item }: { readonly item: PaletteItem }): ReactElement {
  return (
    <>
      <span className="shrink-0 text-fg/50">#{item.number}</span>
      <span className="truncate" title={item.title}>
        {item.title}
      </span>
      <span className="shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs text-fg/60">
        {item.repoName}
      </span>
      <span className="ml-auto shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-xs uppercase text-fg/60">
        {item.type}
      </span>
    </>
  );
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const { dashboard, forceSync } = useGitHubData();
  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });
  const [selectedValue, setSelectedValue] = useState("");

  useEffect(() => {
    if (!open) setSelectedValue("");
  }, [open]);

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();

    const nameCounts = new Map<string, number>();
    for (const repo of repos) {
      nameCounts.set(repo.name, (nameCounts.get(repo.name) ?? 0) + 1);
    }

    return new Map(
      repos.map((repo) => [repo.id, nameCounts.get(repo.name) === 1 ? repo.name : repo.fullName]),
    );
  }, [repos]);

  const { prItems, issueItems } = useMemo(() => {
    if (!dashboard) return { prItems: [], issueItems: [] };

    const allPrItems = deduplicateItems([
      ...dashboard.reviewRequests.map((pr) => prToItem(pr, repoMap, "reviews")),
      ...dashboard.myPullRequests.map((pr) => prToItem(pr, repoMap, "mine")),
    ]);
    const allIssueItems = deduplicateItems(
      dashboard.assignedIssues.map((issue) => issueToItem(issue, repoMap)),
    );

    return { prItems: allPrItems, issueItems: allIssueItems };
  }, [dashboard, repoMap]);

  const allItems = useMemo(() => [...prItems, ...issueItems], [prItems, issueItems]);

  const findSelectedItem = useCallback(
    () => allItems.find((item) => item.id.toLowerCase() === selectedValue) ?? null,
    [allItems, selectedValue],
  );

  function handleSelect(item: PaletteItem): void {
    useDashboardStore.getState().setView(item.section);
    onOpenChange(false);
  }

  function runAction(action: () => void): void {
    action();
    onOpenChange(false);
  }

  function handleKeyDown(e: React.KeyboardEvent): void {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      const item = findSelectedItem();
      if (item) {
        openUrl(item.url);
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
          placeholder="Search PRs, issues, and actions…"
          className={`${FOCUS_RING} w-full border-b border-border bg-transparent px-4 py-3 text-sm text-fg placeholder:text-fg/50`}
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
                  value={item.id}
                  keywords={[String(item.number), item.title, item.repoName]}
                  onSelect={() => handleSelect(item)}
                  className={ITEM_CLASS}
                >
                  <PaletteItemRow item={item} />
                </Command.Item>
              ))}
            </Command.Group>
          )}

          {issueItems.length > 0 && (
            <Command.Group heading="Issues">
              {issueItems.map((item) => (
                <Command.Item
                  key={item.id}
                  value={item.id}
                  keywords={[String(item.number), item.title, item.repoName]}
                  onSelect={() => handleSelect(item)}
                  className={ITEM_CLASS}
                >
                  <PaletteItemRow item={item} />
                </Command.Item>
              ))}
            </Command.Group>
          )}

          <Command.Group heading="Actions">
            <Command.Item
              value="action-dashboard"
              keywords={["dashboard", "home", "overview"]}
              onSelect={() => runAction(() => useDashboardStore.getState().setView("overview"))}
              className={ITEM_CLASS}
            >
              <LayoutDashboard className="size-4 shrink-0 text-fg/50" />
              Navigate to Dashboard
            </Command.Item>
            <Command.Item
              value="action-settings"
              keywords={["settings", "preferences", "config"]}
              onSelect={() => runAction(() => useDashboardStore.getState().setView("settings"))}
              className={ITEM_CLASS}
            >
              <SettingsIcon className="size-4 shrink-0 text-fg/50" />
              Settings
            </Command.Item>
            <Command.Item
              value="action-focus"
              keywords={["focus", "mode", "concentrate", "priority"]}
              onSelect={() => runAction(() => useDashboardStore.getState().toggleFocusMode())}
              className={ITEM_CLASS}
            >
              <Focus className="size-4 shrink-0 text-fg/50" />
              Toggle Focus Mode
            </Command.Item>
            <Command.Item
              value="action-refresh"
              keywords={["refresh", "sync", "reload", "data"]}
              onSelect={() => runAction(() => forceSync())}
              className={ITEM_CLASS}
            >
              <RefreshCw className="size-4 shrink-0 text-fg/50" />
              Refresh Data
            </Command.Item>
            <Command.Item
              value="action-workspaces"
              keywords={["workspace", "workspaces", "open", "terminal"]}
              onSelect={() => runAction(() => useDashboardStore.getState().setView("workspaces"))}
              className={ITEM_CLASS}
            >
              <FolderOpen className="size-4 shrink-0 text-fg/50" />
              Open Workspaces
            </Command.Item>
            <Command.Item
              value="action-docs"
              keywords={["documentation", "docs", "help", "guide"]}
              onSelect={() => runAction(() => openUrl("https://github.com/mpiton/prism"))}
              className={ITEM_CLASS}
            >
              <BookOpen className="size-4 shrink-0 text-fg/50" />
              Open Documentation
            </Command.Item>
            <Command.Item
              value="action-updates"
              keywords={["update", "updates", "version", "release"]}
              onSelect={() => runAction(() => openUrl("https://github.com/mpiton/prism/releases"))}
              className={ITEM_CLASS}
            >
              <Download className="size-4 shrink-0 text-fg/50" />
              Check for Updates
            </Command.Item>
          </Command.Group>
        </Command.List>
      </div>
    </Command.Dialog>
  );
}
