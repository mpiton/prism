import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { CommandPalette } from "./CommandPalette";

vi.mock("../../hooks/useGitHubData", () => ({
  useGitHubData: vi.fn(),
}));

import { useGitHubData } from "../../hooks/useGitHubData";
import type { DashboardData, PullRequestWithReview, Issue } from "../../lib/types";

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

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();

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
    render(<CommandPalette open={false} onOpenChange={() => {}} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("should render dialog and search input when open", () => {
    const { rerender } = render(
      <CommandPalette open={false} onOpenChange={() => {}} />,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    rerender(<CommandPalette open={true} onOpenChange={() => {}} />);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/search/i)).toBeInTheDocument();
  });

  it("should close on Esc", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    render(<CommandPalette open={true} onOpenChange={onOpenChange} />);

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
    render(<CommandPalette open={true} onOpenChange={() => {}} />);

    const input = screen.getByPlaceholderText(/search/i);
    await user.type(input, "login");

    // "Fix login bug" should be visible, "Add search feature" should not
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
    render(<CommandPalette open={true} onOpenChange={() => {}} />);

    const input = screen.getByPlaceholderText(/search/i);
    await user.type(input, "99");

    expect(screen.getByText(/Dashboard crashes on empty data/)).toBeInTheDocument();
  });

  it("should open selected item", async () => {
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
    render(<CommandPalette open={true} onOpenChange={onOpenChange} />);

    const item = screen.getByText(/Fix login bug/);
    await user.click(item);

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
    render(<CommandPalette open={true} onOpenChange={() => {}} />);

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

    render(<CommandPalette open={true} onOpenChange={() => {}} />);

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

    render(<CommandPalette open={true} onOpenChange={() => {}} />);

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

    render(<CommandPalette open={true} onOpenChange={() => {}} />);

    // Should only appear once
    const items = screen.getAllByText(/Fix login bug/);
    expect(items).toHaveLength(1);
  });
});
