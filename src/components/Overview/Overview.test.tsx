import { type ReactElement } from "react";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi, beforeEach } from "vitest";
import type { Activity, Issue } from "../../lib/types/github";
import type { DashboardData, PullRequestWithReview } from "../../lib/types/dashboard";
import { Overview } from "./Overview";

vi.mock("../../lib/open", () => ({
  openUrl: vi.fn(),
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (opts: { count: number; estimateSize: (i: number) => number }) => ({
    getVirtualItems: () =>
      Array.from({ length: opts.count }, (_, i) => ({
        index: i,
        key: i,
        start: i * opts.estimateSize(i),
        size: opts.estimateSize(i),
      })),
    getTotalSize: () => opts.count * opts.estimateSize(0),
  }),
}));

vi.mock("../../hooks/useGitHubData", () => ({
  useGitHubData: vi.fn(),
}));

vi.mock("../../lib/tauri", () => ({
  markAllActivityRead: vi.fn().mockResolvedValue(0),
  listRepos: vi.fn().mockResolvedValue([]),
}));

import { useGitHubData } from "../../hooks/useGitHubData";
import { markAllActivityRead } from "../../lib/tauri";
import { useDashboardStore } from "../../stores/dashboard";

const mockedUseGitHubData = vi.mocked(useGitHubData);
const mockedMarkAllActivityRead = vi.mocked(markAllActivityRead);

function renderWithProviders(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

function makePr(n: number): PullRequestWithReview {
  return {
    pullRequest: {
      id: `pr-${n}`,
      number: n,
      title: `PR #${n}`,
      author: "alice",
      state: "open",
      ciStatus: "success",
      priority: "medium",
      repoId: "repo-1",
      url: `https://github.com/org/repo/pull/${n}`,
      headRefName: "fix/test",
      labels: [],
      additions: 10,
      deletions: 5,
      createdAt: "2026-03-26T10:00:00Z",
      updatedAt: "2026-03-26T12:00:00Z",
    },
    reviewSummary: {
      totalReviews: 1,
      approved: 0,
      changesRequested: 0,
      pending: 1,
      reviewers: ["bob"],
    },
    workspace: null,
  };
}

function makeIssue(n: number): Issue {
  return {
    id: `issue-${n}`,
    number: n,
    title: `Issue #${n}`,
    author: "alice",
    state: "open",
    priority: "medium",
    repoId: "repo-1",
    url: `https://github.com/org/repo/issues/${n}`,
    labels: [],
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
  };
}

function makeActivity(n: number): Activity {
  return {
    id: `activity-${n}`,
    activityType: "comment_added",
    actor: "bob",
    repoId: "repo-1",
    pullRequestId: `pr-${n}`,
    issueId: null,
    message: `Comment on PR #${n}`,
    isRead: false,
    createdAt: "2026-03-26T10:00:00Z",
  };
}

function makeDashboard(overrides: Partial<DashboardData> = {}): DashboardData {
  return {
    reviewRequests: [],
    myPullRequests: [],
    assignedIssues: [],
    recentActivity: [],
    workspaces: [],
    syncedAt: "2026-03-26T12:00:00Z",
    ...overrides,
  };
}

function setupMock(dashboard: DashboardData | null = makeDashboard()) {
  mockedUseGitHubData.mockReturnValue({
    dashboard,
    stats: null,
    isLoading: false,
    error: null,
    authExpired: false,
    syncError: null,
    forceSync: vi.fn(),
    isSyncing: false,
  });
}

beforeEach(() => {
  vi.restoreAllMocks();
  mockedMarkAllActivityRead.mockResolvedValue(0);
  useDashboardStore.setState({
    activeNavigableSection: null,
    navigableSectionRegistrations: [],
    selectedIndex: -1,
    navigableItems: [],
  });
});

describe("Overview", () => {
  it("should render review queue section", () => {
    setupMock(
      makeDashboard({
        reviewRequests: [makePr(1), makePr(2)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("review-queue")).toBeInTheDocument();
    expect(screen.getByText("PR #1")).toBeInTheDocument();
    expect(screen.getByText("PR #2")).toBeInTheDocument();
  });

  it("should use a level-two heading for the priority lane title", () => {
    setupMock(
      makeDashboard({
        reviewRequests: [makePr(1)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(
      screen.getByRole("heading", { name: "Review requests come first", level: 2 }),
    ).toBeInTheDocument();
  });

  it("should render my PRs section", () => {
    setupMock(
      makeDashboard({
        myPullRequests: [makePr(10), makePr(11)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("my-prs")).toBeInTheDocument();
    expect(screen.getByText("PR #10")).toBeInTheDocument();
    expect(screen.getByText("PR #11")).toBeInTheDocument();
  });

  it("should render issues in side panel", () => {
    setupMock(
      makeDashboard({
        assignedIssues: [makeIssue(1), makeIssue(2)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("issues")).toBeInTheDocument();
    expect(screen.getByText("Issue #1")).toBeInTheDocument();
    expect(screen.getByText("Issue #2")).toBeInTheDocument();
  });

  it("should render activity in side panel", () => {
    setupMock(
      makeDashboard({
        recentActivity: [makeActivity(1), makeActivity(2)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();
    expect(screen.getByText("Comment on PR #1")).toBeInTheDocument();
    expect(screen.getByText("Comment on PR #2")).toBeInTheDocument();
  });

  it("should group my PRs and issues inside the secondary grid", () => {
    setupMock(
      makeDashboard({
        myPullRequests: [makePr(10)],
        assignedIssues: [makeIssue(1)],
      }),
    );

    renderWithProviders(<Overview />);

    const secondaryGrid = screen.getByTestId("overview-secondary-grid");

    expect(within(secondaryGrid).getByTestId("my-prs")).toBeInTheDocument();
    expect(within(secondaryGrid).getByTestId("issues")).toBeInTheDocument();
  });

  it("should highlight only the active overview section during keyboard navigation", () => {
    setupMock(
      makeDashboard({
        reviewRequests: [makePr(1)],
        myPullRequests: [makePr(10)],
        assignedIssues: [makeIssue(1)],
      }),
    );

    renderWithProviders(<Overview />);

    act(() => {
      useDashboardStore.setState({
        activeNavigableSection: "reviews",
        selectedIndex: 0,
      });
    });

    expect(document.querySelectorAll('[data-selected="true"]')).toHaveLength(1);
  });

  it("should limit review queue to 5 items", () => {
    const reviews = Array.from({ length: 8 }, (_, i) => makePr(i + 1));
    setupMock(makeDashboard({ reviewRequests: reviews }));

    renderWithProviders(<Overview />);

    expect(screen.getByText("8 in queue")).toBeInTheDocument();
    expect(screen.getByText("PR #5")).toBeInTheDocument();
    expect(screen.queryByText("PR #6")).not.toBeInTheDocument();
  });

  it("should limit my PRs to 5 items", () => {
    const prs = Array.from({ length: 8 }, (_, i) => makePr(i + 10));
    setupMock(makeDashboard({ myPullRequests: prs }));

    renderWithProviders(<Overview />);

    expect(screen.getByText("PR #14")).toBeInTheDocument();
    expect(screen.queryByText("PR #15")).not.toBeInTheDocument();
  });

  it("should limit issues to 5 open items", () => {
    const issues = Array.from({ length: 8 }, (_, i) => makeIssue(i + 1));
    setupMock(makeDashboard({ assignedIssues: issues }));

    renderWithProviders(<Overview />);

    expect(screen.getByText("Issue #5")).toBeInTheDocument();
    expect(screen.queryByText("Issue #6")).not.toBeInTheDocument();
  });

  it("should show 'View all' button when issues exceed limit", () => {
    const issues = Array.from({ length: 8 }, (_, i) => makeIssue(i + 1));
    setupMock(makeDashboard({ assignedIssues: issues }));

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("overview-issues-view-all")).toBeInTheDocument();
    expect(screen.getByTestId("overview-issues-view-all")).toHaveTextContent("View all 8 issues");
  });

  it("should not show 'View all' button when issues fit within limit", () => {
    const issues = Array.from({ length: 3 }, (_, i) => makeIssue(i + 1));
    setupMock(makeDashboard({ assignedIssues: issues }));

    renderWithProviders(<Overview />);

    expect(screen.queryByTestId("overview-issues-view-all")).not.toBeInTheDocument();
  });

  it("should hide 'View all' when many closed but few open issues", () => {
    const openIssues = Array.from({ length: 3 }, (_, i) => makeIssue(i + 1));
    const closedIssues = Array.from({ length: 6 }, (_, i) => ({
      ...makeIssue(i + 10),
      state: "closed" as const,
    }));
    setupMock(makeDashboard({ assignedIssues: [...openIssues, ...closedIssues] }));

    renderWithProviders(<Overview />);

    // Only 3 open issues shown, no closed issues visible
    expect(screen.getByText("Issue #1")).toBeInTheDocument();
    expect(screen.getByText("Issue #3")).toBeInTheDocument();
    expect(screen.queryByText("Issue #10")).not.toBeInTheDocument();
    // "View all" hidden because openIssueCount (3) <= MAX_ISSUES (5)
    expect(screen.queryByTestId("overview-issues-view-all")).not.toBeInTheDocument();
  });

  it("should show 'View all' button when reviews exceed limit", () => {
    const reviews = Array.from({ length: 8 }, (_, i) => makePr(i + 1));
    setupMock(makeDashboard({ reviewRequests: reviews }));

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("overview-reviews-view-all")).toBeInTheDocument();
    expect(screen.getByTestId("overview-reviews-view-all")).toHaveTextContent("View all 8 reviews");
  });

  it("should not show reviews 'View all' button when reviews fit within limit", () => {
    const reviews = Array.from({ length: 3 }, (_, i) => makePr(i + 1));
    setupMock(makeDashboard({ reviewRequests: reviews }));

    renderWithProviders(<Overview />);

    expect(screen.queryByTestId("overview-reviews-view-all")).not.toBeInTheDocument();
  });

  it("should show 'View all' button when PRs exceed limit", () => {
    const prs = Array.from({ length: 8 }, (_, i) => makePr(i + 10));
    setupMock(makeDashboard({ myPullRequests: prs }));

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("overview-prs-view-all")).toBeInTheDocument();
    expect(screen.getByTestId("overview-prs-view-all")).toHaveTextContent("View all 8 PRs");
  });

  it("should not show PRs 'View all' button when PRs fit within limit", () => {
    const prs = Array.from({ length: 3 }, (_, i) => makePr(i + 10));
    setupMock(makeDashboard({ myPullRequests: prs }));

    renderWithProviders(<Overview />);

    expect(screen.queryByTestId("overview-prs-view-all")).not.toBeInTheDocument();
  });

  it("should only show open PRs in overview and exclude merged from count", () => {
    const openPrs = Array.from({ length: 3 }, (_, i) => makePr(i + 10));
    const mergedPrs = Array.from({ length: 4 }, (_, i) => ({
      ...makePr(i + 20),
      pullRequest: { ...makePr(i + 20).pullRequest, state: "merged" as const },
    }));
    setupMock(makeDashboard({ myPullRequests: [...openPrs, ...mergedPrs] }));

    renderWithProviders(<Overview />);

    // Only 3 open PRs shown, merged excluded from preview
    expect(screen.getByText("PR #10")).toBeInTheDocument();
    expect(screen.getByText("PR #12")).toBeInTheDocument();
    expect(screen.queryByText("PR #20")).not.toBeInTheDocument();
    // "View all" hidden because openPrCount (3) <= MAX_PRS (5)
    expect(screen.queryByTestId("overview-prs-view-all")).not.toBeInTheDocument();
    // Badge shows correct open count
    expect(screen.getByText("3 open")).toBeInTheDocument();
  });

  it("should limit activity to 5 items", () => {
    const activities = Array.from({ length: 8 }, (_, i) => makeActivity(i + 1));
    setupMock(makeDashboard({ recentActivity: activities }));

    renderWithProviders(<Overview />);

    expect(screen.getByText("Comment on PR #5")).toBeInTheDocument();
    expect(screen.queryByText("Comment on PR #6")).not.toBeInTheDocument();
  });

  it("should show loading state when dashboard is absent", () => {
    mockedUseGitHubData.mockReturnValue({
      dashboard: null,
      stats: null,
      isLoading: true,
      error: null,
      authExpired: false,
      syncError: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("overview")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("review-queue")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("my-prs")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("issues")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("activity-feed")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByTestId("review-queue-loading")).toBeInTheDocument();
    expect(screen.getByTestId("my-prs-loading")).toBeInTheDocument();
    expect(screen.getByTestId("issues-loading")).toBeInTheDocument();
    expect(screen.getByTestId("activity-feed-loading")).toBeInTheDocument();
  });

  it("should show error state when dashboard is absent and error exists", () => {
    mockedUseGitHubData.mockReturnValue({
      dashboard: null,
      stats: null,
      isLoading: false,
      error: new Error("Network error"),
      authExpired: false,
      syncError: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderWithProviders(<Overview />);

    expect(screen.getByText(/failed to load/i)).toBeInTheDocument();
  });

  it("should render dashboard even when error exists but dashboard is available", () => {
    mockedUseGitHubData.mockReturnValue({
      dashboard: makeDashboard({ reviewRequests: [makePr(1)] }),
      stats: null,
      isLoading: false,
      error: new Error("Stats failed"),
      authExpired: false,
      syncError: null,
      forceSync: vi.fn(),
      isSyncing: false,
    });

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("overview")).toBeInTheDocument();
    expect(screen.getByText("PR #1")).toBeInTheDocument();
  });

  it("should render all 4 sections when data is available", () => {
    setupMock(
      makeDashboard({
        reviewRequests: [makePr(1)],
        myPullRequests: [makePr(10)],
        assignedIssues: [makeIssue(1)],
        recentActivity: [makeActivity(1)],
      }),
    );

    renderWithProviders(<Overview />);

    expect(screen.getByTestId("review-queue")).toBeInTheDocument();
    expect(screen.getByTestId("my-prs")).toBeInTheDocument();
    expect(screen.getByTestId("issues")).toBeInTheDocument();
    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();
  });

  it("should call markAllActivityRead when mark all read is clicked", async () => {
    const user = userEvent.setup();
    setupMock(
      makeDashboard({
        recentActivity: [makeActivity(1)],
      }),
    );

    renderWithProviders(<Overview />);

    await user.click(screen.getByRole("button", { name: /mark all read/i }));

    expect(mockedMarkAllActivityRead).toHaveBeenCalledOnce();
  });

  it("should collapse and expand the activity widget", async () => {
    const user = userEvent.setup();
    setupMock(
      makeDashboard({
        recentActivity: [makeActivity(1)],
      }),
    );

    renderWithProviders(<Overview />);

    const toggle = screen.getByTestId("overview-activity-toggle");

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByTestId("activity-feed")).not.toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();
  });

  it("should open URL when a PR card is clicked", async () => {
    const { openUrl: mockedOpenUrl } = await import("../../lib/open");
    vi.mocked(mockedOpenUrl).mockClear();
    const user = userEvent.setup();
    setupMock(
      makeDashboard({
        reviewRequests: [makePr(1)],
      }),
    );

    renderWithProviders(<Overview />);

    await user.click(screen.getByRole("link"));

    expect(mockedOpenUrl).toHaveBeenCalledWith("https://github.com/org/repo/pull/1");
  });
});
