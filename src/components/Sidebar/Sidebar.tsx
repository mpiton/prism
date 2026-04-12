import { useEffect, useRef, useState } from "react";
import type { ReactElement } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useDashboardStore } from "../../stores/dashboard";
import type { DashboardView } from "../../stores/dashboard";
import { useWorkspacesStore } from "../../stores/workspaces";
import { useGitHubData } from "../../hooks/useGitHubData";
import { FOCUS_RING } from "../../lib/a11y";
import { listNotifications, listRepos, setRepoEnabled, authGetStatus } from "../../lib/tauri";
import { NavItem } from "./NavItem";
import { WorkspaceList } from "./WorkspaceList";
import { RepoList } from "./RepoList";

interface NavEntry {
  readonly label: string;
  readonly view: DashboardView;
  readonly countKey?:
    | "pendingReviews"
    | "openPrs"
    | "openIssues"
    | "totalWorkspaces"
    | "unreadActivity";
}

const NAV_ITEMS: readonly NavEntry[] = [
  { label: "Overview", view: "overview" },
  { label: "To Review", view: "reviews", countKey: "pendingReviews" },
  { label: "My PRs", view: "mine", countKey: "openPrs" },
  { label: "Issues", view: "issues", countKey: "openIssues" },
  { label: "Activity", view: "feed", countKey: "unreadActivity" },
  { label: "Notifications", view: "notifications" },
  { label: "Workspaces", view: "workspaces", countKey: "totalWorkspaces" },
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

  const notificationsQuery = useQuery({
    queryKey: ["github", "notifications"],
    queryFn: listNotifications,
    staleTime: 60_000,
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

  const [isReposExpanded, setIsReposExpanded] = useState(false);
  const [focusedView, setFocusedView] = useState<DashboardView>(currentView);
  const navGroupRef = useRef<HTMLDivElement | null>(null);
  const navItemRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const workspaces = (dashboard?.workspaces ?? []).filter((ws) => ws.state !== "archived");
  const repos = reposQuery.data ?? [];
  const enabledRepos = repos.filter((r) => r.enabled);
  const username = authQuery.data?.username ?? null;
  const unreadNotificationsCount = notificationsQuery.data?.filter((item) => item.unread).length;

  useEffect(() => {
    if (typeof document !== "undefined" && navGroupRef.current?.contains(document.activeElement)) {
      return;
    }
    setFocusedView(currentView);
  }, [currentView]);

  async function handleSelectAll() {
    const toEnable = repos.filter((r) => !r.enabled);
    const results = await Promise.allSettled(toEnable.map((r) => setRepoEnabled(r.id, true)));
    await queryClient.invalidateQueries({ queryKey: ["repos"] });
    const failures = results.filter((r) => r.status === "rejected");
    if (failures.length > 0) {
      console.error("[Sidebar] batch enable failed for", failures.length, "repos");
    }
  }

  async function handleDeselectAll() {
    const toDisable = repos.filter((r) => r.enabled);
    const results = await Promise.allSettled(toDisable.map((r) => setRepoEnabled(r.id, false)));
    await queryClient.invalidateQueries({ queryKey: ["repos"] });
    const failures = results.filter((r) => r.status === "rejected");
    if (failures.length > 0) {
      console.error("[Sidebar] batch disable failed for", failures.length, "repos");
    }
  }

  function focusNavItem(index: number) {
    const item = NAV_ITEMS[index];
    if (!item) return;
    navItemRefs.current[index]?.focus();
  }

  function handleNavKeyDown(event: React.KeyboardEvent<HTMLButtonElement>, index: number) {
    const count = NAV_ITEMS.length;
    let nextIndex: number | null = null;

    switch (event.key) {
      case "ArrowDown":
        nextIndex = (index + 1) % count;
        break;
      case "ArrowUp":
        nextIndex = (index - 1 + count) % count;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = count - 1;
        break;
      default:
        return;
    }

    event.preventDefault();
    focusNavItem(nextIndex);
  }

  return (
    <nav
      data-testid="sidebar"
      aria-label="Main navigation"
      className="flex h-full flex-col gap-4 p-3"
    >
      {/* Logo */}
      <div className="px-2 py-1">
        <h1 className="text-sm font-bold text-white">PRism</h1>
        <p className="text-xs text-dim">GitHub Review Dashboard</p>
      </div>

      {/* Navigation */}
      <div
        ref={navGroupRef}
        className="flex flex-col gap-0.5"
        role="group"
        aria-label="Primary views"
      >
        {NAV_ITEMS.map((item, index) => (
          <NavItem
            key={item.view}
            buttonRef={(element) => {
              navItemRefs.current[index] = element;
            }}
            label={item.label}
            view={item.view}
            count={
              item.view === "notifications"
                ? unreadNotificationsCount
                : item.countKey && stats
                  ? stats[item.countKey]
                  : undefined
            }
            isActive={currentView === item.view}
            tabIndex={focusedView === item.view ? 0 : -1}
            onClick={handleNavClick}
            onFocus={() => setFocusedView(item.view)}
            onKeyDown={(event) => handleNavKeyDown(event, index)}
          />
        ))}
      </div>

      {/* Workspaces section */}
      {workspaces.length > 0 && (
        <div
          role="region"
          aria-labelledby="sidebar-workspaces-heading"
          className="flex flex-col gap-1"
        >
          <h3
            id="sidebar-workspaces-heading"
            className="px-2 text-[10px] font-semibold uppercase tracking-wider text-dim"
          >
            Workspaces
          </h3>
          <WorkspaceList workspaces={workspaces} onWorkspaceClick={handleWorkspaceClick} />
        </div>
      )}

      {/* Repos section — collapsible; full management in Settings */}
      {repos.length > 0 && (
        <div
          role="region"
          aria-label={`Repos ${enabledRepos.length}`}
          className="flex min-h-0 flex-col gap-1"
        >
          <button
            type="button"
            aria-expanded={isReposExpanded}
            aria-label={`Repos ${enabledRepos.length}`}
            onClick={() => setIsReposExpanded((prev) => !prev)}
            className={`${FOCUS_RING} flex w-full items-center justify-between rounded px-2 text-[10px] font-semibold uppercase tracking-wider text-dim hover:text-foreground`}
          >
            <span id="sidebar-repos-heading" className="inline-flex items-baseline">
              <span>Repos</span>
              <span className="text-dim/60"> {enabledRepos.length}</span>
            </span>
            <span aria-hidden="true">{isReposExpanded ? "▾" : "▸"}</span>
          </button>
          {isReposExpanded && (
            <div className="max-h-[200px] overflow-y-auto">
              <RepoList
                repos={repos}
                onToggleRepo={handleToggleRepo}
                onSelectAll={handleSelectAll}
                onDeselectAll={handleDeselectAll}
              />
            </div>
          )}
        </div>
      )}

      {/* Focus Mode Toggle */}
      <div className="px-2">
        <button
          type="button"
          aria-pressed={focusMode}
          onClick={toggleFocusMode}
          className={`${FOCUS_RING} w-full rounded px-2 py-2 text-xs font-medium ${
            focusMode
              ? "bg-accent text-bg font-semibold hover:bg-accent/80"
              : "text-dim hover:text-foreground"
          }`}
        >
          Focus
        </button>
      </div>

      {/* Footer */}
      <div className="mt-auto border-t border-border px-2 pt-2">
        {username && <p className="truncate text-xs text-dim">{username}</p>}
        <p className="text-[10px] text-dim/60">⌘K</p>
      </div>
    </nav>
  );
}
