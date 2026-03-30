import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import type { Workspace } from "../../lib/types";
import { useWorkspacesStore } from "../../stores/workspaces";

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
] as const;

function resetStore() {
  useWorkspacesStore.setState({
    activeWorkspaceId: "ws-1",
  });
}

describe("WorkspaceSwitcher", () => {
  beforeEach(() => {
    resetStore();
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

    expect(activeDot.className).toContain("bg-green");
    expect(suspendedDot.className).toContain("bg-orange");
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
});
