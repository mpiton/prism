import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Sidebar } from "./Sidebar";
import { useDashboardStore } from "../../stores/dashboard";
import { useWorkspacesStore } from "../../stores/workspaces";

vi.mock("../../hooks/useGitHubData", () => ({
  useGitHubData: () => ({
    stats: {
      pendingReviews: 3,
      openPrs: 7,
      openIssues: 2,
      totalWorkspaces: 1,
      unreadActivity: 4,
    },
    dashboard: {
      reviewRequests: [],
      myPullRequests: [],
      assignedIssues: [],
      recentActivity: [],
      workspaces: [
        {
          id: "ws-1",
          repoId: "repo-1",
          pullRequestNumber: 42,
          state: "active",
          worktreePath: "/tmp/ws-1",
          sessionId: "session-1",
          createdAt: "2026-03-28T10:00:00Z",
          updatedAt: "2026-03-28T10:00:00Z",
        },
        {
          id: "ws-2",
          repoId: "repo-2",
          pullRequestNumber: 99,
          state: "suspended",
          worktreePath: "/tmp/ws-2",
          sessionId: null,
          createdAt: "2026-03-28T09:00:00Z",
          updatedAt: "2026-03-28T09:00:00Z",
        },
      ],
      syncedAt: "2026-03-28T10:00:00Z",
    },
    isLoading: false,
    error: null,
    forceSync: vi.fn(),
    isSyncing: false,
  }),
}));

vi.mock("../../lib/tauri", () => ({
  listRepos: vi.fn().mockResolvedValue([
    {
      id: "repo-1",
      org: "acme",
      name: "frontend",
      fullName: "acme/frontend",
      url: "https://github.com/acme/frontend",
      defaultBranch: "main",
      isArchived: false,
      enabled: true,
      localPath: "/home/user/frontend",
      lastSyncAt: "2026-03-28T10:00:00Z",
    },
  ]),
  setRepoEnabled: vi.fn().mockResolvedValue({}),
  authGetStatus: vi.fn().mockResolvedValue({
    connected: true,
    username: "matvei",
    error: null,
  }),
}));

function renderSidebar() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <Sidebar />
    </QueryClientProvider>,
  );
}

describe("Sidebar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useDashboardStore.setState({ currentView: "overview", activeFilters: {}, focusMode: false });
    useWorkspacesStore.setState({ activeWorkspaceId: null });
  });

  it("should render logo", () => {
    renderSidebar();
    expect(screen.getByText("PRism")).toBeInTheDocument();
  });

  it("should highlight active view", () => {
    useDashboardStore.setState({ currentView: "reviews" });
    renderSidebar();
    const reviewButton = screen.getByRole("button", { name: /to review/i });
    expect(reviewButton).toHaveAttribute("aria-current", "page");
  });

  it("should show review count", () => {
    renderSidebar();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("should show workspace dots with state colors", () => {
    const { container } = renderSidebar();
    const dots = container.querySelectorAll("[data-state]");
    expect(dots.length).toBeGreaterThanOrEqual(2);
    const states = Array.from(dots).map((d) => d.getAttribute("data-state"));
    expect(states).toContain("active");
    expect(states).toContain("suspended");
  });

  it("should toggle repos", async () => {
    const { setRepoEnabled } = await import("../../lib/tauri");
    renderSidebar();

    // Repos are collapsed by default — expand first
    const reposHeader = await screen.findByRole("button", { name: /repos/i });
    await userEvent.click(reposHeader);

    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).toBeChecked();

    await userEvent.click(checkbox);
    expect(vi.mocked(setRepoEnabled)).toHaveBeenCalledWith("repo-1", false);
  });

  it("should switch to workspace on click", async () => {
    renderSidebar();
    const wsEntry = screen.getByText(/PR #42/);
    await userEvent.click(wsEntry);

    expect(useDashboardStore.getState().currentView).toBe("workspaces");
    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-1");
  });

  it("should render all navigation items", () => {
    renderSidebar();
    expect(screen.getByRole("button", { name: /overview/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /to review/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /my prs/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /issues/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /activity/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /workspaces/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /settings/i })).toBeInTheDocument();
  });

  it("should navigate when nav item is clicked", async () => {
    renderSidebar();
    await userEvent.click(screen.getByRole("button", { name: /issues/i }));
    expect(useDashboardStore.getState().currentView).toBe("issues");
  });

  it("should toggle focus mode via button", async () => {
    renderSidebar();

    const focusBtn = screen.getByRole("button", { name: /focus/i });
    expect(focusBtn).toBeInTheDocument();

    expect(useDashboardStore.getState().focusMode).toBe(false);
    await userEvent.click(focusBtn);
    expect(useDashboardStore.getState().focusMode).toBe(true);
    await userEvent.click(focusBtn);
    expect(useDashboardStore.getState().focusMode).toBe(false);
  });

  it("should show focus mode button as active when focus mode is on", () => {
    useDashboardStore.setState({ focusMode: true });
    renderSidebar();

    const focusBtn = screen.getByRole("button", { name: /focus/i });
    expect(focusBtn).toHaveAttribute("aria-pressed", "true");
  });

  it("should show footer with keyboard shortcut", () => {
    renderSidebar();
    expect(screen.getByText(/⌘K/)).toBeInTheDocument();
  });

  it("should have aria-label on nav items with counts", () => {
    renderSidebar();
    const reviewButton = screen.getByRole("button", { name: /to review \(3\)/i });
    expect(reviewButton).toBeInTheDocument();
  });

  it("should have aria-label on workspaces section", () => {
    renderSidebar();
    const section = screen.getByRole("region", { name: /workspaces/i });
    expect(section).toBeInTheDocument();
  });

  it("should have aria-label on repos section", async () => {
    renderSidebar();
    const section = await screen.findByRole("region", { name: /repos/i });
    expect(section).toBeInTheDocument();
  });

  it("should show repos section collapsed by default", async () => {
    renderSidebar();
    const reposHeader = await screen.findByRole("button", { name: /repos/i });
    expect(reposHeader).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByPlaceholderText("Filter repos...")).not.toBeInTheDocument();
  });

  it("should expand repos section when header is clicked", async () => {
    renderSidebar();
    const reposHeader = await screen.findByRole("button", { name: /repos/i });
    await userEvent.click(reposHeader);
    expect(reposHeader).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByPlaceholderText("Filter repos...")).toBeInTheDocument();
  });

  it("should collapse repos section when header is clicked again", async () => {
    renderSidebar();
    const reposHeader = await screen.findByRole("button", { name: /repos/i });
    await userEvent.click(reposHeader);
    await userEvent.click(reposHeader);
    expect(reposHeader).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByPlaceholderText("Filter repos...")).not.toBeInTheDocument();
  });

  it("should show repo count in collapsed header", async () => {
    renderSidebar();
    // Wait for repos to load — the header shows the enabled count
    const reposHeader = await screen.findByRole("button", { name: /repos/i });
    expect(reposHeader).toBeInTheDocument();
    // The mock provides 1 enabled repo — exact match to avoid false positives
    expect(reposHeader).toHaveAccessibleName("Repos 1");
    expect(reposHeader).toHaveTextContent(/^Repos 1\s*▸$/);
  });
});
