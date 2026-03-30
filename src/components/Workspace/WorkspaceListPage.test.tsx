import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceListPage } from "./WorkspaceListPage";
import type { WorkspaceListEntry } from "../../lib/types";

// ── Mock atoms ──────────────────────────────────────────────────

vi.mock("../atoms/CI", () => ({
  CI: ({ status }: { status: string }) => (
    <span data-testid="ci-badge">{status}</span>
  ),
}));

vi.mock("../atoms/EmptyState", () => ({
  EmptyState: ({ message }: { message: string }) => (
    <div data-testid="empty-state">{message}</div>
  ),
}));

// ── Fixtures ────────────────────────────────────────────────────

const ACTIVE_ENTRY: WorkspaceListEntry = {
  workspace: {
    id: "ws-1",
    repoId: "repo-1",
    pullRequestNumber: 42,
    state: "active",
    worktreePath: null,
    sessionId: null,
    createdAt: "2026-03-01T10:00:00Z",
    updatedAt: "2026-03-28T14:30:00Z",
  },
  branch: "feat/login",
  ahead: 3,
  behind: 1,
  ciStatus: "success",
  sessionCount: 5,
  diskUsageMb: 120,
  lastNote: "Fixed auth flow, needs final review",
};

const SUSPENDED_ENTRY: WorkspaceListEntry = {
  workspace: {
    id: "ws-2",
    repoId: "repo-1",
    pullRequestNumber: 99,
    state: "suspended",
    worktreePath: null,
    sessionId: null,
    createdAt: "2026-02-15T08:00:00Z",
    updatedAt: "2026-03-20T09:00:00Z",
  },
  branch: "fix/bug-99",
  ahead: 0,
  behind: 4,
  ciStatus: "failure",
  sessionCount: 2,
  diskUsageMb: 85,
  lastNote: null,
};

const ARCHIVED_ENTRY: WorkspaceListEntry = {
  workspace: {
    id: "ws-3",
    repoId: "repo-2",
    pullRequestNumber: 7,
    state: "archived",
    worktreePath: null,
    sessionId: null,
    createdAt: "2026-01-10T08:00:00Z",
    updatedAt: "2026-02-01T12:00:00Z",
  },
  branch: null,
  ahead: 0,
  behind: 0,
  ciStatus: null,
  sessionCount: 0,
  diskUsageMb: null,
  lastNote: null,
};

const ENTRIES: readonly WorkspaceListEntry[] = [ACTIVE_ENTRY, SUSPENDED_ENTRY];

// ── Tests ───────────────────────────────────────────────────────

describe("WorkspaceListPage", () => {
  it("should display PR numbers for all entries", () => {
    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/PR #42/)).toBeInTheDocument();
    expect(screen.getByText(/PR #99/)).toBeInTheDocument();
  });

  it("should show state dot with correct data attribute", () => {
    const { container } = render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    const dots = container.querySelectorAll("[data-state]");
    expect(dots).toHaveLength(2);
    expect(dots[0]).toHaveAttribute("data-state", "active");
    expect(dots[1]).toHaveAttribute("data-state", "suspended");
  });

  it("should show archived state dot", () => {
    const { container } = render(
      <WorkspaceListPage entries={[ARCHIVED_ENTRY]} onWorkspaceClick={vi.fn()} />,
    );

    const dot = container.querySelector("[data-state='archived']");
    expect(dot).toBeInTheDocument();
  });

  it("should show disk usage total", () => {
    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    // 120 + 85 = 205 MB total
    expect(screen.getByText(/205\s*MB/i)).toBeInTheDocument();
  });

  it("should show 0 MB total when all entries have diskUsageMb 0", () => {
    const zeroEntries: readonly WorkspaceListEntry[] = [
      { ...ACTIVE_ENTRY, diskUsageMb: 0 },
    ];

    render(
      <WorkspaceListPage entries={zeroEntries} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/Total: 0 MB/)).toBeInTheDocument();
  });

  it("should call onWorkspaceClick with workspace id on click", async () => {
    const user = userEvent.setup();
    const handleClick = vi.fn();

    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={handleClick} />,
    );

    await user.click(screen.getByRole("button", { name: /PR #42/ }));
    expect(handleClick).toHaveBeenCalledWith("ws-1");
  });

  it("should show empty state when no workspaces", () => {
    render(
      <WorkspaceListPage entries={[]} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByTestId("empty-state")).toBeInTheDocument();
  });

  it("should show branch and ahead/behind info", () => {
    render(
      <WorkspaceListPage entries={[ACTIVE_ENTRY]} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/feat\/login/)).toBeInTheDocument();
    expect(screen.getByText("+3")).toBeInTheDocument();
    expect(screen.getByText("-1")).toBeInTheDocument();
  });

  it("should show CI status badge", () => {
    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    const badges = screen.getAllByTestId("ci-badge");
    expect(badges).toHaveLength(2);
    expect(badges[0]).toHaveTextContent("success");
    expect(badges[1]).toHaveTextContent("failure");
  });

  it("should show session count", () => {
    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/5 sessions/)).toBeInTheDocument();
    expect(screen.getByText(/2 sessions/)).toBeInTheDocument();
  });

  it("should show last note excerpt when available", () => {
    render(
      <WorkspaceListPage entries={ENTRIES} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/Fixed auth flow/)).toBeInTheDocument();
  });

  it("should use singular form for 1 session", () => {
    const singleSession: readonly WorkspaceListEntry[] = [
      { ...ACTIVE_ENTRY, sessionCount: 1 },
    ];

    render(
      <WorkspaceListPage entries={singleSession} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.getByText(/1 session$/)).toBeInTheDocument();
  });

  it("should not show disk usage footer when no entries have disk data", () => {
    const entriesNoDisk: readonly WorkspaceListEntry[] = [
      { ...ACTIVE_ENTRY, diskUsageMb: null },
    ];

    render(
      <WorkspaceListPage entries={entriesNoDisk} onWorkspaceClick={vi.fn()} />,
    );

    expect(screen.queryByText(/Total:/)).not.toBeInTheDocument();
  });
});
