import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Workspace } from "../../lib/types";
import { WorkspaceList } from "./WorkspaceList";

const activeWs: Workspace = {
  id: "ws-1",
  repoId: "repo-1",
  pullRequestNumber: 42,
  state: "active",
  worktreePath: "/tmp/ws-1",
  sessionId: "session-1",
  createdAt: "2026-03-28T10:00:00Z",
  updatedAt: "2026-03-28T10:00:00Z",
};

const suspendedWs: Workspace = {
  id: "ws-2",
  repoId: "repo-2",
  pullRequestNumber: 99,
  state: "suspended",
  worktreePath: "/tmp/ws-2",
  sessionId: null,
  createdAt: "2026-03-28T09:00:00Z",
  updatedAt: "2026-03-28T09:00:00Z",
};

const archivedWs: Workspace = {
  id: "ws-3",
  repoId: "repo-1",
  pullRequestNumber: 10,
  state: "archived",
  worktreePath: null,
  sessionId: null,
  createdAt: "2026-03-27T08:00:00Z",
  updatedAt: "2026-03-27T08:00:00Z",
};

describe("WorkspaceList", () => {
  it("should render workspace entries", () => {
    render(
      <WorkspaceList workspaces={[activeWs, suspendedWs]} onWorkspaceClick={vi.fn()} />,
    );
    expect(screen.getByText(/PR #42/)).toBeInTheDocument();
    expect(screen.getByText(/PR #99/)).toBeInTheDocument();
  });

  it("should show workspace dots with state colors", () => {
    const { container } = render(
      <WorkspaceList workspaces={[activeWs, suspendedWs]} onWorkspaceClick={vi.fn()} />,
    );
    const dots = container.querySelectorAll("[data-state]");
    expect(dots).toHaveLength(2);
    expect(dots[0]).toHaveAttribute("data-state", "active");
    expect(dots[1]).toHaveAttribute("data-state", "suspended");
  });

  it("should not render archived workspaces", () => {
    render(
      <WorkspaceList
        workspaces={[activeWs, archivedWs]}
        onWorkspaceClick={vi.fn()}
      />,
    );
    expect(screen.getByText(/PR #42/)).toBeInTheDocument();
    expect(screen.queryByText(/PR #10/)).not.toBeInTheDocument();
  });

  it("should call onWorkspaceClick with workspace id on click", async () => {
    const handleClick = vi.fn();
    render(
      <WorkspaceList workspaces={[activeWs]} onWorkspaceClick={handleClick} />,
    );
    await userEvent.click(screen.getByText(/PR #42/));
    expect(handleClick).toHaveBeenCalledWith("ws-1");
  });

  it("should render empty when no workspaces", () => {
    const { container } = render(
      <WorkspaceList workspaces={[]} onWorkspaceClick={vi.fn()} />,
    );
    expect(container.querySelectorAll("[data-state]")).toHaveLength(0);
  });
});
