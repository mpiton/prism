import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createElement, type ReactNode } from "react";
import { useGitHubData } from "./useGitHubData";
import type { DashboardData, DashboardStats } from "../lib/types";

vi.mock("../lib/tauri", () => ({
  getGithubDashboard: vi.fn(),
  getGithubStats: vi.fn(),
  forceGithubSync: vi.fn(),
  onEvent: vi.fn(),
}));

import {
  getGithubDashboard,
  getGithubStats,
  forceGithubSync,
  onEvent,
} from "../lib/tauri";

const MOCK_DASHBOARD: DashboardData = {
  reviewRequests: [],
  myPullRequests: [],
  assignedIssues: [],
  recentActivity: [],
  workspaces: [],
  syncedAt: "2026-03-27T10:00:00Z",
};

const MOCK_STATS: DashboardStats = {
  pendingReviews: 3,
  openPrs: 5,
  openIssues: 2,
  totalWorkspaces: 1,
  unreadActivity: 7,
};

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return {
    queryClient,
    wrapper: ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client: queryClient }, children),
  };
}

describe("useGitHubData", () => {
  let unlistenFn: Mock;

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenFn = vi.fn();
    (onEvent as Mock).mockResolvedValue(unlistenFn);
  });

  it("should fetch dashboard data", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useGitHubData(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.dashboard).toEqual(MOCK_DASHBOARD);
    expect(result.current.stats).toEqual(MOCK_STATS);
    expect(result.current.error).toBeNull();
  });

  it("should return loading state", () => {
    (getGithubDashboard as Mock).mockReturnValue(new Promise(() => {}));
    (getGithubStats as Mock).mockReturnValue(new Promise(() => {}));

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useGitHubData(), { wrapper });

    expect(result.current.isLoading).toBe(true);
    expect(result.current.dashboard).toBeNull();
    expect(result.current.stats).toBeNull();
  });

  it("should invalidate on github:updated event", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useGitHubData(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "github:updated",
        expect.any(Function),
      );
    });

    // Simulate the event firing
    const call = (onEvent as Mock).mock.calls[0];
    expect(call).toBeDefined();
    const eventCallback = call![1] as () => void;
    await act(() => {
      eventCallback();
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "dashboard"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "stats"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["repos"],
    });
  });

  it("should force sync via mutation", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);
    (forceGithubSync as Mock).mockResolvedValue(undefined);

    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    const { result } = renderHook(() => useGitHubData(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(() => {
      result.current.forceSync();
    });

    await waitFor(() => {
      expect(forceGithubSync).toHaveBeenCalledOnce();
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "dashboard"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "stats"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["repos"],
    });
  });

  it("should call unlisten on unmount", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper } = createWrapper();
    const { unmount } = renderHook(() => useGitHubData(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledTimes(4);
    });

    unmount();
    // All 4 listeners should be cleaned up (updated, expired, restored, sync_error)
    expect(unlistenFn).toHaveBeenCalledTimes(4);
  });

  it("should set authExpired when auth:expired event fires", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useGitHubData(), { wrapper });

    expect(result.current.authExpired).toBe(false);

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "auth:expired",
        expect.any(Function),
      );
    });

    // Find the auth:expired callback
    const authCall = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "auth:expired",
    );
    expect(authCall).toBeDefined();
    const authCallback = authCall![1] as (payload: string) => void;

    await act(() => {
      authCallback("invalid or expired token");
    });

    expect(result.current.authExpired).toBe(true);
  });

  it("should disable queries when authExpired", async () => {
    vi.useFakeTimers();

    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useGitHubData(100), { wrapper });

    await vi.waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    // Clear call counts before triggering auth:expired
    (getGithubDashboard as Mock).mockClear();
    (getGithubStats as Mock).mockClear();

    // Trigger auth:expired
    const authCall = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "auth:expired",
    );
    const authCallback = authCall![1] as (payload: string) => void;

    await act(() => {
      authCallback("invalid or expired token");
    });

    expect(result.current.authExpired).toBe(true);

    // Advance past the refetch interval to verify queries don't fire
    await act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(getGithubDashboard).not.toHaveBeenCalled();
    expect(getGithubStats).not.toHaveBeenCalled();

    vi.useRealTimers();
  });

  it("should reset authExpired when auth:restored fires", async () => {
    (getGithubDashboard as Mock).mockResolvedValue(MOCK_DASHBOARD);
    (getGithubStats as Mock).mockResolvedValue(MOCK_STATS);

    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");
    const { result } = renderHook(() => useGitHubData(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "auth:expired",
        expect.any(Function),
      );
    });

    // First: trigger auth:expired
    const expiredCall = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "auth:expired",
    );
    await act(() => {
      (expiredCall![1] as (p: string) => void)("token expired");
    });
    expect(result.current.authExpired).toBe(true);

    // Then: trigger auth:restored
    const restoredCall = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "auth:restored",
    );
    expect(restoredCall).toBeDefined();

    invalidateSpy.mockClear();
    await act(() => {
      (restoredCall![1] as (p: string) => void)("octocat");
    });

    expect(result.current.authExpired).toBe(false);
    // Should invalidate queries to refetch fresh data
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "dashboard"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "stats"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["repos"],
    });
  });
});
