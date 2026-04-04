import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import type { Workspace } from "../../lib/types";
import { useWorkspacesStore } from "../../stores/workspaces";
import { useSettingsStore } from "../../stores/settings";

// ── Mock data ───────────────────────────────────────────────────────

const WORKSPACES: readonly Workspace[] = [
  {
    id: "ws-1",
    repoId: "repo-1",
    pullRequestNumber: 42,
    state: "active",
    worktreePath: "/tmp/ws-1",
    sessionId: null,
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
  {
    id: "ws-3",
    repoId: "repo-2",
    pullRequestNumber: 7,
    state: "active",
    worktreePath: "/tmp/ws-3",
    sessionId: null,
    createdAt: "2026-03-01T00:00:00Z",
    updatedAt: "2026-03-01T00:00:00Z",
  },
  {
    id: "ws-4",
    repoId: "repo-2",
    pullRequestNumber: 15,
    state: "archived",
    worktreePath: null,
    sessionId: null,
    createdAt: "2026-03-01T00:00:00Z",
    updatedAt: "2026-03-01T00:00:00Z",
  },
] as const;

function resetStores() {
  useWorkspacesStore.setState({ activeWorkspaceId: "ws-1" });
  useSettingsStore.setState({
    config: {
      pollIntervalSecs: 300,
      maxActiveWorkspaces: 3,
      autoSuspendMinutes: 30,
      githubToken: null,
      dataDir: null,
      workspacesDir: null,
    },
  });
}

describe("WorkspaceSwitcher", () => {
  beforeEach(() => {
    resetStores();
  });

  it("should render all workspace tabs", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByText("#42")).toBeInTheDocument();
    expect(screen.getByText("#99")).toBeInTheDocument();
    expect(screen.getByText("#7")).toBeInTheDocument();
  });

  it("should highlight active tab", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const activeTab = screen.getByTestId("tab-ws-1");
    const inactiveTab = screen.getByTestId("tab-ws-2");

    expect(activeTab.getAttribute("data-active")).toBe("true");
    expect(inactiveTab.getAttribute("data-active")).toBe("false");
  });

  it("should switch workspace on click", async () => {
    const user = userEvent.setup();

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const tab = screen.getByTestId("tab-ws-2");
    await user.click(tab);

    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-2");
  });

  it("should show active count", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    // 2 active out of 3 max
    expect(screen.getByText("2/3")).toBeInTheDocument();
  });

  it("should show state dot with correct color", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const activeDot = screen.getByTestId("dot-ws-1");
    const suspendedDot = screen.getByTestId("dot-ws-2");
    const archivedDot = screen.getByTestId("dot-ws-4");

    expect(activeDot.className).toContain("bg-green");
    expect(suspendedDot.className).toContain("bg-orange");
    expect(archivedDot.className).toContain("bg-dim");
  });

  it("should use maxActiveWorkspaces from settings store", () => {
    useSettingsStore.setState({
      config: {
        pollIntervalSecs: 300,
        maxActiveWorkspaces: 5,
        autoSuspendMinutes: 30,
        githubToken: null,
        dataDir: null,
        workspacesDir: null,
      },
    });

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByText("2/5")).toBeInTheDocument();
  });

  it("should have proper ARIA tab roles", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(4);

    const activeTab = screen.getByTestId("tab-ws-1");
    const inactiveTab = screen.getByTestId("tab-ws-2");

    expect(activeTab).toHaveAttribute("aria-selected", "true");
    expect(inactiveTab).toHaveAttribute("aria-selected", "false");
  });

  it("should call onBackToDashboard when back button is clicked", async () => {
    const user = userEvent.setup();
    const onBack = vi.fn();

    render(
      <WorkspaceSwitcher workspaces={WORKSPACES} onBackToDashboard={onBack} />,
    );

    const backBtn = screen.getByRole("button", { name: /dashboard/i });
    await user.click(backBtn);

    expect(onBack).toHaveBeenCalledOnce();
  });

  it("should fallback to default when maxActiveWorkspaces is invalid", () => {
    useSettingsStore.setState({
      config: {
        pollIntervalSecs: 300,
        maxActiveWorkspaces: 0,
        autoSuspendMinutes: 30,
        githubToken: null,
        dataDir: null,
        workspacesDir: null,
      },
    });

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    expect(screen.getByText("2/3")).toBeInTheDocument();
  });

  it("should navigate tabs with ArrowRight key", async () => {
    const user = userEvent.setup();

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const firstTab = screen.getByTestId("tab-ws-1");
    firstTab.focus();
    await user.keyboard("{ArrowRight}");

    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-2");
  });

  it("should navigate tabs with ArrowLeft key (wrap around)", async () => {
    const user = userEvent.setup();

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const firstTab = screen.getByTestId("tab-ws-1");
    firstTab.focus();
    await user.keyboard("{ArrowLeft}");

    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-4");
  });

  it("should navigate to first tab with Home key", async () => {
    const user = userEvent.setup();
    useWorkspacesStore.setState({ activeWorkspaceId: "ws-3" });

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const tab = screen.getByTestId("tab-ws-3");
    tab.focus();
    await user.keyboard("{Home}");

    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-1");
  });

  it("should navigate to last tab with End key", async () => {
    const user = userEvent.setup();

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const firstTab = screen.getByTestId("tab-ws-1");
    firstTab.focus();
    await user.keyboard("{End}");

    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-4");
  });

  it("should set tabIndex 0 on active tab and -1 on others", () => {
    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const activeTab = screen.getByTestId("tab-ws-1");
    const inactiveTab = screen.getByTestId("tab-ws-2");

    expect(activeTab).toHaveAttribute("tabindex", "0");
    expect(inactiveTab).toHaveAttribute("tabindex", "-1");
  });

  it("should fallback first tab to tabIndex 0 when activeWorkspaceId is null", () => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });

    render(
      <WorkspaceSwitcher
        workspaces={WORKSPACES}
        onBackToDashboard={vi.fn()}
      />,
    );

    const firstTab = screen.getByTestId("tab-ws-1");
    const secondTab = screen.getByTestId("tab-ws-2");

    expect(firstTab).toHaveAttribute("tabindex", "0");
    expect(secondTab).toHaveAttribute("tabindex", "-1");
  });
});
