import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createElement, type ReactNode } from "react";
import { useWorkspace } from "./useWorkspace";
import type { Workspace } from "../lib/types/workspace";
import type { WorkspaceState } from "../lib/types/enums";

vi.mock("../lib/tauri", () => ({
  listWorkspaces: vi.fn(),
  onEvent: vi.fn(),
}));

vi.mock("../stores/workspaces", () => ({
  useWorkspacesStore: vi.fn(),
}));

import { listWorkspaces, onEvent } from "../lib/tauri";
import { useWorkspacesStore } from "../stores/workspaces";

const MOCK_WORKSPACES: readonly Workspace[] = [
  {
    id: "ws-1",
    repoId: "repo-1",
    pullRequestNumber: 42,
    state: "active",
    worktreePath: "/tmp/ws-1",
    sessionId: null,
    createdAt: "2026-03-30T10:00:00Z",
    updatedAt: "2026-03-30T10:00:00Z",
  },
  {
    id: "ws-2",
    repoId: "repo-1",
    pullRequestNumber: 43,
    state: "suspended",
    worktreePath: "/tmp/ws-2",
    sessionId: "session-abc",
    createdAt: "2026-03-30T09:00:00Z",
    updatedAt: "2026-03-30T09:00:00Z",
  },
];

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

function getEventHandler<T>(eventName: string): (payload: T) => void {
  const call = (onEvent as Mock).mock.calls.find(
    (c: unknown[]) => c[0] === eventName,
  );
  if (!call) throw new Error(`No onEvent call registered for "${eventName}"`);
  return call[1] as (payload: T) => void;
}

describe("useWorkspace", () => {
  let unlistenStateChanged: Mock;
  let unlistenClaudeSession: Mock;

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenStateChanged = vi.fn();
    unlistenClaudeSession = vi.fn();
    (onEvent as Mock)
      .mockResolvedValueOnce(unlistenStateChanged)
      .mockResolvedValueOnce(unlistenClaudeSession);
    (listWorkspaces as Mock).mockResolvedValue(MOCK_WORKSPACES);
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: null }),
    );
  });

  it("should fetch workspaces via TanStack Query", async () => {
    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.workspaces).toEqual(MOCK_WORKSPACES);
    expect(listWorkspaces).toHaveBeenCalledOnce();
  });

  it("should update on state_changed event", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["workspaces"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "dashboard"],
    });
  });

  it("should invalidate queries on claude_session event", async () => {
    const { wrapper, queryClient } = createWrapper();
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:claude_session",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; sessionId: string }>(
      "workspace:claude_session",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", sessionId: "session-xyz" });
    });

    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["workspaces"],
    });
    expect(invalidateSpy).toHaveBeenCalledWith({
      queryKey: ["github", "dashboard"],
    });
  });

  it("should show message when active workspace suspended", async () => {
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: "ws-1" }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBe("ws-1");
    });
  });

  it("should clear suspended message after acknowledgement", async () => {
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: "ws-1" }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBe("ws-1");
    });

    act(() => {
      result.current.dismissSuspendedNotice();
    });

    expect(result.current.suspendedActiveWorkspace).toBeNull();
  });

  it("should call unlisten on unmount", async () => {
    const { wrapper } = createWrapper();
    const { unmount } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledTimes(2);
    });

    unmount();

    await waitFor(() => {
      expect(unlistenStateChanged).toHaveBeenCalledOnce();
      expect(unlistenClaudeSession).toHaveBeenCalledOnce();
    });
  });

  it("should not flag suspended workspace when it is not the active one", async () => {
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: "ws-2" }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    expect(result.current.suspendedActiveWorkspace).toBeNull();
  });

  it("should clear suspended notice when workspace resumes", async () => {
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: "ws-1" }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBe("ws-1");
    });

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "active" });
    });

    expect(result.current.suspendedActiveWorkspace).toBeNull();
  });

  it("should clear suspended notice when workspace is archived", async () => {
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: "ws-1" }),
    );

    const { wrapper } = createWrapper();
    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBe("ws-1");
    });

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "archived" });
    });

    expect(result.current.suspendedActiveWorkspace).toBeNull();
  });

  it("should clear stale notice when active workspace changes", async () => {
    let currentActiveId: string | null = "ws-1";
    (useWorkspacesStore as unknown as Mock).mockImplementation((selector: (s: { activeWorkspaceId: string | null }) => unknown) =>
      selector({ activeWorkspaceId: currentActiveId }),
    );

    const { wrapper } = createWrapper();
    const { result, rerender } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:state_changed",
        expect.any(Function),
      );
    });

    const handler = getEventHandler<{ workspaceId: string; newState: WorkspaceState }>(
      "workspace:state_changed",
    );

    await act(() => {
      handler({ workspaceId: "ws-1", newState: "suspended" });
    });

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBe("ws-1");
    });

    // Switch active workspace
    currentActiveId = "ws-2";
    rerender();

    await waitFor(() => {
      expect(result.current.suspendedActiveWorkspace).toBeNull();
    });
  });
});
