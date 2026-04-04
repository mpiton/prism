import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useWorkspacesStore } from "../../stores/workspaces";
import type { Workspace, WorkspaceListEntry, WorkspaceState, WorkspaceStatusInfo } from "../../lib/types";

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

vi.mock("./WorkspaceListPage", () => ({
  WorkspaceListPage: ({
    entries,
    onWorkspaceClick,
  }: {
    entries: readonly WorkspaceListEntry[];
    onWorkspaceClick: (id: string) => void;
  }) => (
    <div data-testid="workspace-list">
      {entries.map((e) => (
        <button
          key={e.workspace.id}
          data-testid={`workspace-item-${e.workspace.id}`}
          onClick={() => onWorkspaceClick(e.workspace.id)}
        >
          PR #{e.workspace.pullRequestNumber}
        </button>
      ))}
    </div>
  ),
}));

vi.mock("../../lib/tauri", () => ({
  resumeWorkspace: vi.fn(),
}));

import { WorkspaceView } from "./WorkspaceView";
import { resumeWorkspace } from "../../lib/tauri";

// ── Test helpers ─────────────────────────────────────────────────

function makeQueryClient(): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

function renderWithQuery(ui: React.ReactElement): ReturnType<typeof render> {
  const queryClient = makeQueryClient();
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

function makeEntry(
  overrides: Partial<Omit<WorkspaceListEntry, "workspace">> & {
    workspaceOverrides?: Partial<Workspace> & { state?: WorkspaceState };
  } = {},
): WorkspaceListEntry {
  const { workspaceOverrides, ...rest } = overrides;
  const { state = "active", ...wsRest } = workspaceOverrides ?? {};
  return {
    workspace: {
      id: `ws-${Math.random().toString(36).slice(2, 8)}`,
      repoId: "repo-1",
      pullRequestNumber: 42,
      state,
      worktreePath: "/tmp/ws",
      sessionId: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      ...wsRest,
    },
    branch: "feat/test",
    ahead: 0,
    behind: 0,
    ciStatus: null,
    githubUrl: "https://github.com/test/repo/pull/42",
    sessionCount: 1,
    diskUsageMb: 50,
    lastNote: null,
    ...rest,
  };
}

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

const STATUS_INFO = {
  "ws-1": {
    branch: "feat/login",
    ahead: 2,
    behind: 0,
    ciStatus: "success",
    sessionName: "prism-pr-42",
    sessionCount: 3,
    githubUrl: "https://github.com/test/repo/pull/42",
  },
  "ws-2": {
    branch: "fix/bug-99",
    ahead: 0,
    behind: 1,
    ciStatus: "pending",
    sessionName: null,
    sessionCount: 0,
    githubUrl: "https://github.com/test/repo/pull/99",
  },
} satisfies Readonly<Record<string, WorkspaceStatusInfo>>;

describe("WorkspaceView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useWorkspacesStore.setState({ activeWorkspaceId: "ws-1" });
  });

  it("should render switcher, terminal, and status bar", () => {
    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        entries={[]}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-ws-1")).toBeInTheDocument();
    expect(screen.getByTestId("workspace-statusbar")).toBeInTheDocument();
  });

  it("should switch terminal on workspace change", () => {
    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        entries={[]}
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

  it("should render terminal without status bar when status info is missing", () => {
    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={{}}
        entries={[]}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-ws-1")).toBeInTheDocument();
    expect(screen.queryByTestId("workspace-statusbar")).not.toBeInTheDocument();
  });

  it("should show empty state when active workspace not in list", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: "ws-unknown" });

    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={STATUS_INFO}
        entries={[]}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-switcher")).toBeInTheDocument();
    expect(screen.queryByTestId("workspace-statusbar")).not.toBeInTheDocument();
    expect(screen.getByTestId("workspace-list")).toBeInTheDocument();
  });

  it("should render WorkspaceListPage when no active workspace", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    const activeEntry = makeEntry({ workspaceOverrides: { id: "ws-a", state: "active", pullRequestNumber: 10 } });
    const suspendedEntry = makeEntry({ workspaceOverrides: { id: "ws-b", state: "suspended", pullRequestNumber: 20 } });

    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={{}}
        entries={[activeEntry, suspendedEntry]}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-list")).toBeInTheDocument();
    expect(screen.queryByTestId("terminal-ws-1")).not.toBeInTheDocument();
  });

  it("should filter out archived workspaces from list", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    const activeEntry = makeEntry({ workspaceOverrides: { id: "ws-active", state: "active", pullRequestNumber: 1 } });
    const archivedEntry = makeEntry({ workspaceOverrides: { id: "ws-archived", state: "archived", pullRequestNumber: 2 } });

    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={{}}
        entries={[activeEntry, archivedEntry]}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByTestId("workspace-item-ws-active")).toBeInTheDocument();
    expect(screen.queryByTestId("workspace-item-ws-archived")).not.toBeInTheDocument();
  });

  it("should set active workspace when clicking an active workspace", async () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    const setActiveWorkspace = vi.fn();
    useWorkspacesStore.setState({ setActiveWorkspace } as Partial<ReturnType<typeof useWorkspacesStore.getState>>);

    const activeEntry = makeEntry({ workspaceOverrides: { id: "ws-click", state: "active", pullRequestNumber: 5 } });

    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={{}}
        entries={[activeEntry]}
        onBackToDashboard={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByTestId("workspace-item-ws-click"));

    await waitFor(() => {
      expect(resumeWorkspace).not.toHaveBeenCalled();
      expect(setActiveWorkspace).toHaveBeenCalledWith("ws-click");
    });
  });

  it("should call resumeWorkspace then set active when clicking a suspended workspace", async () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    const callOrder: string[] = [];
    const setActiveWorkspace = vi.fn(() => callOrder.push("setActive"));
    useWorkspacesStore.setState({ setActiveWorkspace } as Partial<ReturnType<typeof useWorkspacesStore.getState>>);

    vi.mocked(resumeWorkspace).mockImplementation(async () => {
      callOrder.push("resume");
    });

    const suspendedEntry = makeEntry({ workspaceOverrides: { id: "ws-suspended", state: "suspended", pullRequestNumber: 7 } });

    renderWithQuery(
      <WorkspaceView
        workspaces={WORKSPACES}
        statusInfo={{}}
        entries={[suspendedEntry]}
        onBackToDashboard={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByTestId("workspace-item-ws-suspended"));

    await waitFor(() => {
      expect(resumeWorkspace).toHaveBeenCalledWith("ws-suspended");
      expect(setActiveWorkspace).toHaveBeenCalledWith("ws-suspended");
      expect(callOrder).toEqual(["resume", "setActive"]);
    });
  });
});
