import type { ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useDashboardStore } from "../../stores/dashboard";
import type { DashboardView } from "../../stores/dashboard";
import { useWorkspacesStore } from "../../stores/workspaces";
import { useGitHubData } from "../../hooks/useGitHubData";
import { listRepos, setRepoEnabled, authGetStatus } from "../../lib/tauri";
import { NavItem } from "./NavItem";
import { WorkspaceList } from "./WorkspaceList";
import { RepoList } from "./RepoList";

interface NavEntry {
  readonly label: string;
  readonly view: DashboardView;
  readonly countKey?: "pendingReviews" | "openPrs" | "openIssues" | "activeWorkspaces" | "unreadActivity";
}

const NAV_ITEMS: readonly NavEntry[] = [
  { label: "Overview", view: "overview" },
  { label: "To Review", view: "reviews", countKey: "pendingReviews" },
  { label: "My PRs", view: "mine", countKey: "openPrs" },
  { label: "Issues", view: "issues", countKey: "openIssues" },
  { label: "Activity", view: "feed", countKey: "unreadActivity" },
  { label: "Workspaces", view: "workspaces", countKey: "activeWorkspaces" },
  { label: "Settings", view: "settings" },
];

export function Sidebar(): ReactElement {
  const queryClient = useQueryClient();
  const currentView = useDashboardStore((s) => s.currentView);
  const setView = useDashboardStore((s) => s.setView);
  const focusMode = useDashboardStore((s) => s.focusMode);
  const toggleFocusMode = useDashboardStore((s) => s.toggleFocusMode);
  const setActiveWorkspace = useWorkspacesStore((s) => s.setActiveWorkspace);
  const { stats, dashboard } = useGitHubData();

  const reposQuery = useQuery({
    queryKey: ["repos"],
    queryFn: listRepos,
  });

  const authQuery = useQuery({
    queryKey: ["auth", "status"],
    queryFn: authGetStatus,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });

  const toggleRepoMutation = useMutation({
    mutationFn: ({ repoId, enabled }: { repoId: string; enabled: boolean }) =>
      setRepoEnabled(repoId, enabled),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
    onError: async () => {
      await queryClient.invalidateQueries({ queryKey: ["repos"] });
    },
  });

  function handleNavClick(view: DashboardView) {
    setView(view);
  }

  function handleWorkspaceClick(workspaceId: string) {
    setActiveWorkspace(workspaceId);
    setView("workspaces");
  }

  function handleToggleRepo(repoId: string, enabled: boolean) {
    toggleRepoMutation.mutate({ repoId, enabled });
  }

  const workspaces = (dashboard?.workspaces ?? []).filter(
    (ws) => ws.state !== "archived",
  );
  const repos = reposQuery.data ?? [];
  const username = authQuery.data?.username ?? null;

  return (
    <nav data-testid="sidebar" aria-label="Main navigation" className="flex h-full flex-col gap-4 p-3">
      {/* Logo */}
      <div className="px-2 py-1">
        <h1 className="text-sm font-bold text-white">PRism</h1>
        <p className="text-xs text-dim">GitHub Review Dashboard</p>
      </div>

      {/* Navigation */}
      <div className="flex flex-col gap-0.5">
        {NAV_ITEMS.map((item) => (
          <NavItem
            key={item.view}
            label={item.label}
            view={item.view}
            count={item.countKey && stats ? stats[item.countKey] : undefined}
            isActive={currentView === item.view}
            onClick={handleNavClick}
          />
        ))}
      </div>

      {/* Workspaces section */}
      {workspaces.length > 0 && (
        <div role="region" aria-labelledby="sidebar-workspaces-heading" className="flex flex-col gap-1">
          <h3 id="sidebar-workspaces-heading" className="px-2 text-[10px] font-semibold uppercase tracking-wider text-dim">
            Workspaces
          </h3>
          <WorkspaceList
            workspaces={workspaces}
            onWorkspaceClick={handleWorkspaceClick}
          />
        </div>
      )}

      {/* Repos section */}
      {repos.length > 0 && (
        <div role="region" aria-labelledby="sidebar-repos-heading" className="flex flex-col gap-1">
          <h3 id="sidebar-repos-heading" className="px-2 text-[10px] font-semibold uppercase tracking-wider text-dim">
            Repos
          </h3>
          <RepoList repos={repos} onToggleRepo={handleToggleRepo} />
        </div>
      )}

      {/* Focus Mode Toggle */}
      <div className="px-2">
        <button
          type="button"
          aria-pressed={focusMode}
          onClick={toggleFocusMode}
          className={`w-full rounded px-2 py-1 text-xs font-medium ${
            focusMode
              ? "bg-accent text-white"
              : "text-dim hover:text-foreground"
          }`}
        >
          Focus
        </button>
      </div>

      {/* Footer */}
      <div className="mt-auto border-t border-border px-2 pt-2">
        {username && (
          <p className="truncate text-xs text-dim">{username}</p>
        )}
        <p className="text-[10px] text-dim/60">⌘K</p>
      </div>
    </nav>
  );
}
