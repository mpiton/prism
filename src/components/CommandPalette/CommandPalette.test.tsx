import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CommandPalette } from "./CommandPalette";

vi.mock("../../hooks/useGitHubData", () => ({
  useGitHubData: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  listRepos: vi.fn(),
}));

const mockSetView = vi.fn();
vi.mock("../../stores/dashboard", () => ({
  useDashboardStore: { getState: () => ({ setView: mockSetView }) },
  DashboardView: {},
}));

import { useGitHubData } from "../../hooks/useGitHubData";
import { listRepos } from "../../lib/tauri";
import type { DashboardData, PullRequestWithReview, Issue } from "../../lib/types";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

function renderPalette(props: { open: boolean; onOpenChange: (open: boolean) => void }) {
  return render(
    <QueryClientProvider client={queryClient}>
      <CommandPalette {...props} />
    </QueryClientProvider>,
  );
}

function makePr(overrides: Partial<PullRequestWithReview> = {}): PullRequestWithReview {
  return {
    pullRequest: {
      id: "pr-1",
      number: 42,
      title: "Fix login bug",
      author: "alice",
      state: "open",
      ciStatus: "success",
      priority: "high",
      repoId: "repo-1",
      url: "https://github.com/org/repo/pull/42",
      labels: [],
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    },
    reviewSummary: {
      totalReviews: 1,
      approved: 0,
      changesRequested: 0,
      pending: 1,
      reviewers: ["bob"],
    },
    workspace: null,
    ...overrides,
  };
}

function makeIssue(overrides: Partial<Issue> = {}): Issue {
  return {
    id: "issue-1",
    number: 99,
    title: "Dashboard crashes on empty data",
    author: "carol",
    state: "open",
    priority: "medium",
    repoId: "repo-1",
    url: "https://github.com/org/repo/issues/99",
    labels: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function makeDashboard(
  overrides: Partial<DashboardData> = {},
): DashboardData {
  return {
    reviewRequests: [],
    myPullRequests: [],
    assignedIssues: [],
    recentActivity: [],
    workspaces: [],
    syncedAt: null,
    ...overrides,
  };
}

const mockRepo = {
  id: "repo-1",
  name: "prism",
  org: "mpiton",
  fullName: "mpiton/prism",
  url: "",
  defaultBranch: "main",
  isArchived: false,
  enabled: true,
  localPath: null,
  lastSyncAt: null,
};

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    queryClient.clear();
    mockSetView.mockReset();

    (listRepos as Mock).mockResolvedValue([mockRepo]);

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard(),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });
  });

  it("should not render when closed", () => {
    renderPalette({ open: false, onOpenChange: () => {} });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("should render dialog and search input when open", () => {
    const { rerender } = renderPalette({
      open: false,
      onOpenChange: () => {},
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    rerender(
      <QueryClientProvider client={queryClient}>
        <CommandPalette open={true} onOpenChange={() => {}} />
      </QueryClientProvider>,
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument();
  });

  it("should close on Esc", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderPalette({ open: true, onOpenChange });

    await user.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("should search PRs by title", async () => {
    const pr1 = makePr();
    const pr2 = makePr({
      pullRequest: {
        ...makePr().pullRequest,
        id: "pr-2",
        number: 43,
        title: "Add search feature",
      },
    });

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({
        reviewRequests: [pr1],
        myPullRequests: [pr2],
      }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    const user = userEvent.setup();
    renderPalette({ open: true, onOpenChange: () => {} });

    const input = screen.getByPlaceholderText(/search/i);
    await user.type(input, "login");

    expect(screen.getByText(/Fix login bug/)).toBeInTheDocument();
    expect(screen.queryByText(/Add search feature/)).not.toBeInTheDocument();
  });

  it("should search issues by number", async () => {
    const issue = makeIssue();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ assignedIssues: [issue] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    const user = userEvent.setup();
    renderPalette({ open: true, onOpenChange: () => {} });

    const input = screen.getByPlaceholderText(/search/i);
    await user.type(input, "99");

    expect(screen.getByText(/Dashboard crashes on empty data/)).toBeInTheDocument();
  });

  it("should navigate to reviews section when selecting a PR", async () => {
    const pr = makePr();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ reviewRequests: [pr] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderPalette({ open: true, onOpenChange });

    const item = screen.getByText(/Fix login bug/);
    await user.click(item);

    expect(mockSetView).toHaveBeenCalledWith("reviews");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("should navigate to issues section when selecting an issue", async () => {
    const issue = makeIssue();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ assignedIssues: [issue] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderPalette({ open: true, onOpenChange });

    const item = screen.getByText(/Dashboard crashes on empty data/);
    await user.click(item);

    expect(mockSetView).toHaveBeenCalledWith("issues");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("should open selected item in browser on Cmd+Enter", async () => {
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
    const pr = makePr();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ reviewRequests: [pr] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderPalette({ open: true, onOpenChange });

    // Navigate to item via keyboard (arrow down selects first item)
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Meta>}{Enter}{/Meta}");

    expect(openSpy).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/42",
      "_blank",
      "noopener,noreferrer",
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);

    openSpy.mockRestore();
  });

  it("should show empty state when no results match", async () => {
    const user = userEvent.setup();
    renderPalette({ open: true, onOpenChange: () => {} });

    const input = screen.getByPlaceholderText(/search/i);
    await user.type(input, "zzzznonexistent");

    expect(screen.getByText(/no results/i)).toBeInTheDocument();
  });

  it("should display PR items with number, title and repo info", () => {
    const pr = makePr();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ reviewRequests: [pr] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderPalette({ open: true, onOpenChange: () => {} });

    expect(screen.getByText(/Fix login bug/)).toBeInTheDocument();
    expect(screen.getByText(/#42/)).toBeInTheDocument();
  });

  it("should display issue items with number and title", () => {
    const issue = makeIssue();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ assignedIssues: [issue] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderPalette({ open: true, onOpenChange: () => {} });

    expect(screen.getByText(/Dashboard crashes on empty data/)).toBeInTheDocument();
    expect(screen.getByText(/#99/)).toBeInTheDocument();
  });

  it("should deduplicate PRs that appear in both review and my lists", () => {
    const pr = makePr();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({
        reviewRequests: [pr],
        myPullRequests: [pr],
      }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderPalette({ open: true, onOpenChange: () => {} });

    const items = screen.getAllByText(/Fix login bug/);
    expect(items).toHaveLength(1);
  });

  it("should display repo name for each item", async () => {
    const pr = makePr();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({ reviewRequests: [pr] }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderPalette({ open: true, onOpenChange: () => {} });

    // Wait for the query to resolve
    expect(await screen.findByText("prism")).toBeInTheDocument();
  });

  it("should group items under Pull Requests and Issues headings", () => {
    const pr = makePr();
    const issue = makeIssue();

    (useGitHubData as Mock).mockReturnValue({
      dashboard: makeDashboard({
        reviewRequests: [pr],
        assignedIssues: [issue],
      }),
      stats: null,
      isLoading: false,
      error: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderPalette({ open: true, onOpenChange: () => {} });

    expect(screen.getByText("Pull Requests")).toBeInTheDocument();
    expect(screen.getByText("Issues")).toBeInTheDocument();
  });
});
