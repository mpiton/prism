import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { useWorkspacesStore } from "../../stores/workspaces";
import type { Workspace, CiStatus } from "../../lib/types";

// ── Mock child components ────────────────────────────────────────

vi.mock("./WorkspaceSwitcher", () => ({
  WorkspaceSwitcher: ({
    workspaces,
  }: {
    workspaces: readonly Workspace[];
    onBackToDashboard: () => void;
  }) => (
    <div data-testid="workspace-switcher">
      {workspaces.length} tabs
    </div>
  ),
}));

vi.mock("./Terminal", () => ({
  Terminal: ({ ptyId }: { ptyId: string }) => (
    <div data-testid={`terminal-${ptyId}`} />
  ),
}));

vi.mock("./WorkspaceStatusBar", () => ({
  WorkspaceStatusBar: ({ workspaceId }: { workspaceId: string }) => (
    <div data-testid="workspace-statusbar">{workspaceId}</div>
  ),
}));

import { WorkspaceView } from "./WorkspaceView";
import type { WorkspaceStatusInfo } from "./WorkspaceView";

// ── Mock data ────────────────────────────────────────────────────

const WORKSPACES: readonly Workspace[] = [
  {
    id: "ws-1",
    repoId: "repo-1",
    pullRequestNumber: 42,
    state: "active",
    worktreePath: "/tmp/ws-1",
    sessionId: "session-1",
    createdAt: "2026-03-01T00:00:00Z",
    updatedAt: "2026-03-01T00:00:00Z",
  },
  {
    id: "ws-2",
    repoId: "repo-1",
    pullRequestNumber: 99,
    state: "suspended",
    worktreePath: "/tmp/ws-2",
    sessionId: null,
    createdAt: "2026-03-01T00:00:00Z",
    updatedAt: "2026-03-01T00:00:00Z",
  },
];

const STATUS_INFO: Readonly<Record<string, WorkspaceStatusInfo>> = {
  "ws-1": {
    branch: "feat/login",
    ahead: 2,
    behind: 0,
    ciStatus: "success" as CiStatus,
    sessionName: "prism-pr-42",
    sessionCount: 3,
    githubUrl: "https://github.com/test/repo/pull/42",
  },
  "ws-2": {
    branch: "fix/bug-99",
    ahead: 0,
    behind: 1,
    ciStatus: "pending" as CiStatus,
    sessionName: null,
    sessionCount: 0,
    githubUrl: "https://github.com/test/repo/pull/99",
  },
};

describe("WorkspaceView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useWorkspacesStore.setState({ activeWorkspaceId: "ws-1" });
  });

  it("should render switcher, terminal, and status bar", () => {
    render(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-ws-1")).toBeInTheDocument();
    expect(screen.getByTestId("workspace-statusbar")).toBeInTheDocument();
  });

  it("should switch terminal on workspace change", () => {
    render(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("terminal-ws-1")).toBeInTheDocument();

    act(() => {
      useWorkspacesStore.setState({ activeWorkspaceId: "ws-2" });
    });

    expect(screen.getByTestId("terminal-ws-2")).toBeInTheDocument();
    expect(screen.queryByTestId("terminal-ws-1")).not.toBeInTheDocument();
  });

  it("should show empty state when no active workspace", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    render(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.queryByTestId("terminal-ws-1")).not.toBeInTheDocument();
    expect(screen.getByText(/select a workspace/i)).toBeInTheDocument();
  });

  it("should show empty state when active workspace not in list", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: "ws-unknown" });

    render(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.queryByTestId("workspace-statusbar")).not.toBeInTheDocument();
  });
});
