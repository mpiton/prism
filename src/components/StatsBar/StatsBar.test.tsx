import { describe, expect, it, vi, beforeEach, type Mock } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createElement, type ReactNode } from "react";
import { StatsBar } from "./StatsBar";
import { useDashboardStore } from "../../stores/dashboard";
import type { DashboardStats } from "../../lib/types";

vi.mock("../../hooks/useGitHubData", () => ({
  useGitHubData: vi.fn(),
}));

import { useGitHubData } from "../../hooks/useGitHubData";

const MOCK_STATS: DashboardStats = {
  pendingReviews: 3,
  openPrs: 5,
  openIssues: 2,
  activeWorkspaces: 1,
  unreadActivity: 7,
};

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
}

describe("StatsBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("should render all 4 stats", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: "2026-03-28T10:00:00Z" },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();

    expect(screen.getByText(/pending reviews/i)).toBeInTheDocument();
    expect(screen.getByText(/open prs/i)).toBeInTheDocument();
    expect(screen.getByText(/issues/i)).toBeInTheDocument();
    expect(screen.getByText(/workspaces/i)).toBeInTheDocument();
  });

  it("should show synced time", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: new Date(Date.now() - 30_000).toISOString() },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText(/synced.*ago/i)).toBeInTheDocument();
  });

  it("should highlight pending reviews", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: "2026-03-28T10:00:00Z" },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    const pendingValue = screen.getByTestId("stat-pending-reviews-value");
    expect(pendingValue).toHaveClass("text-accent");
  });

  it("should show placeholder when stats are null", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: null,
      dashboard: null,
      isLoading: true,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    const values = screen.getAllByText("—");
    expect(values.length).toBe(4);
  });

  it("should show minutes format when synced over 60s ago", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: new Date(Date.now() - 120_000).toISOString() },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText(/synced 2m ago/i)).toBeInTheDocument();
  });

  it("should show hours format when synced over 60m ago", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: new Date(Date.now() - 7_200_000).toISOString() },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText(/synced 2h ago/i)).toBeInTheDocument();
  });

  it("should show 'never synced' when syncedAt is null", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: null },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText(/never synced/i)).toBeInTheDocument();
  });

  it("should show FOCUS MODE indicator when focus mode is on", () => {
    useDashboardStore.setState({ focusMode: true });
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: "2026-03-28T10:00:00Z" },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText("FOCUS MODE")).toBeInTheDocument();
  });

  it("should not show FOCUS MODE indicator when focus mode is off", () => {
    useDashboardStore.setState({ focusMode: false });
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: "2026-03-28T10:00:00Z" },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.queryByText("FOCUS MODE")).not.toBeInTheDocument();
  });

  it("should show 'never synced' when syncedAt is an invalid date", () => {
    (useGitHubData as Mock).mockReturnValue({
      stats: MOCK_STATS,
      dashboard: { syncedAt: "not-a-date" },
      isLoading: false,
      error: null,
    });

    render(<StatsBar />, { wrapper: createWrapper() });

    expect(screen.getByText(/never synced/i)).toBeInTheDocument();
  });
});
