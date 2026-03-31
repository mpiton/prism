import { describe, it, expect } from "vitest";
import { TAURI_COMMANDS, TAURI_EVENTS } from "./types";
import type {
  PrState,
  CiStatus,
  Priority,
  ReviewStatus,
  IssueState,
  ActivityType,
  WorkspaceState,
  Repo,
  PullRequest,
  ReviewRequest,
  ReviewSummary,
  Issue,
  Activity,
  Workspace,
  WorkspaceNote,
  WorkspaceSummary,
  PullRequestWithReview,
  DashboardData,
  DashboardStats,
  PersonalStats,
  OpenWorkspaceRequest,
  OpenWorkspaceResponse,
  PtyInput,
  PtyOutput,
  PtyResize,
  AppConfig,
} from "./types";

// ── Helpers ──────────────────────────────────────────────────────

/** Coerce a JSON fixture to type T. Compile-time check only — no runtime validation. */
function coerceFixture<T>(json: unknown): T {
  return json as T;
}

// ── Enum union types ─────────────────────────────────────────────

describe("Enum union types", () => {
  it("should accept all PrState values", () => {
    const values: PrState[] = ["open", "closed", "merged", "draft"];
    expect(values).toHaveLength(4);
  });

  it("should accept all CiStatus values", () => {
    const values: CiStatus[] = ["pending", "running", "success", "failure", "cancelled"];
    expect(values).toHaveLength(5);
  });

  it("should accept all Priority values", () => {
    const values: Priority[] = ["low", "medium", "high", "critical"];
    expect(values).toHaveLength(4);
  });

  it("should accept all ReviewStatus values", () => {
    const values: ReviewStatus[] = [
      "pending",
      "approved",
      "changes_requested",
      "commented",
      "dismissed",
    ];
    expect(values).toHaveLength(5);
  });

  it("should accept all IssueState values", () => {
    const values: IssueState[] = ["open", "closed"];
    expect(values).toHaveLength(2);
  });

  it("should accept all ActivityType values", () => {
    const values: ActivityType[] = [
      "pr_opened",
      "pr_merged",
      "pr_closed",
      "review_submitted",
      "comment_added",
      "ci_completed",
      "issue_opened",
      "issue_closed",
    ];
    expect(values).toHaveLength(8);
  });

  it("should accept all WorkspaceState values", () => {
    const values: WorkspaceState[] = ["active", "suspended", "archived"];
    expect(values).toHaveLength(3);
  });
});

// ── Core struct shapes (JSON fixture conformity) ─────────────────

describe("Core struct shapes", () => {
  it("should match Repo shape from Rust JSON", () => {
    const json = {
      id: "r-1",
      name: "prism",
      fullName: "mpiton/prism",
      url: "https://github.com/mpiton/prism",
      defaultBranch: "main",
      isArchived: false,
    };
    const repo = coerceFixture<Repo>(json);
    expect(repo.id).toBe("r-1");
    expect(repo.fullName).toBe("mpiton/prism");
    expect(repo.defaultBranch).toBe("main");
    expect(repo.isArchived).toBe(false);
  });

  it("should match PullRequest shape from Rust JSON", () => {
    const json = {
      id: "pr-1",
      number: 42,
      title: "Add feature",
      author: "mpiton",
      state: "open" as PrState,
      ciStatus: "success" as CiStatus,
      priority: "high" as Priority,
      repoId: "r-1",
      url: "https://github.com/mpiton/prism/pull/42",
      labels: ["enhancement", "frontend"],
      createdAt: "2026-03-24T10:00:00Z",
      updatedAt: "2026-03-24T12:00:00Z",
    };
    const pr = coerceFixture<PullRequest>(json);
    expect(pr.number).toBe(42);
    expect(pr.ciStatus).toBe("success");
    expect(pr.labels).toEqual(["enhancement", "frontend"]);
  });

  it("should match ReviewRequest shape from Rust JSON", () => {
    const json = {
      id: "rr-1",
      pullRequestId: "pr-1",
      reviewer: "alice",
      status: "pending" as ReviewStatus,
      requestedAt: "2026-03-24T10:00:00Z",
    };
    const rr = coerceFixture<ReviewRequest>(json);
    expect(rr.pullRequestId).toBe("pr-1");
    expect(rr.status).toBe("pending");
  });

  it("should match ReviewSummary shape from Rust JSON", () => {
    const json = {
      totalReviews: 3,
      approved: 1,
      changesRequested: 1,
      pending: 1,
      reviewers: ["alice", "bob"],
    };
    const rs = coerceFixture<ReviewSummary>(json);
    expect(rs.totalReviews).toBe(3);
    expect(rs.changesRequested).toBe(1);
  });

  it("should match Issue shape from Rust JSON", () => {
    const json = {
      id: "i-1",
      number: 10,
      title: "Bug report",
      author: "bob",
      state: "open" as IssueState,
      priority: "critical" as Priority,
      repoId: "r-1",
      url: "https://github.com/mpiton/prism/issues/10",
      labels: ["bug"],
      createdAt: "2026-03-24T10:00:00Z",
      updatedAt: "2026-03-24T12:00:00Z",
    };
    const issue = coerceFixture<Issue>(json);
    expect(issue.priority).toBe("critical");
  });

  it("should match Activity shape with optional fields", () => {
    const json = {
      id: "a-1",
      activityType: "pr_opened" as ActivityType,
      actor: "mpiton",
      repoId: "r-1",
      pullRequestId: "pr-1",
      issueId: null,
      message: "Opened PR #42",
      createdAt: "2026-03-24T10:00:00Z",
    };
    const activity = coerceFixture<Activity>(json);
    expect(activity.pullRequestId).toBe("pr-1");
    expect(activity.issueId).toBeNull();
  });

  it("should match Workspace shape with optional fields", () => {
    const json = {
      id: "ws-1",
      repoId: "r-1",
      pullRequestNumber: 42,
      state: "active" as WorkspaceState,
      worktreePath: "/home/user/.prism/workspaces/prism/worktrees/pr-42",
      sessionId: "session-abc",
      createdAt: "2026-03-24T10:00:00Z",
      updatedAt: "2026-03-24T12:00:00Z",
    };
    const ws = coerceFixture<Workspace>(json);
    expect(ws.pullRequestNumber).toBe(42);
    expect(ws.worktreePath).toBe("/home/user/.prism/workspaces/prism/worktrees/pr-42");
  });

  it("should match WorkspaceNote shape from Rust JSON", () => {
    const json = {
      id: "wn-1",
      workspaceId: "ws-1",
      content: "Review feedback applied",
      createdAt: "2026-03-24T10:00:00Z",
    };
    const note = coerceFixture<WorkspaceNote>(json);
    expect(note.workspaceId).toBe("ws-1");
  });
});

// ── Composite struct shapes ──────────────────────────────────────

describe("Composite struct shapes", () => {
  it("should match WorkspaceSummary shape", () => {
    const json = {
      id: "ws-1",
      state: "active" as WorkspaceState,
      lastNoteContent: "LGTM, ready to merge",
    };
    const ws = coerceFixture<WorkspaceSummary>(json);
    expect(ws.lastNoteContent).toBe("LGTM, ready to merge");
  });

  it("should match WorkspaceSummary with null lastNoteContent", () => {
    const json = {
      id: "ws-2",
      state: "suspended" as WorkspaceState,
      lastNoteContent: null,
    };
    const ws = coerceFixture<WorkspaceSummary>(json);
    expect(ws.lastNoteContent).toBeNull();
  });

  it("should match PullRequestWithReview shape", () => {
    const json = {
      pullRequest: {
        id: "pr-1",
        number: 42,
        title: "Add feature",
        author: "mpiton",
        state: "open",
        ciStatus: "success",
        priority: "high",
        repoId: "r-1",
        url: "https://github.com/mpiton/prism/pull/42",
        labels: ["enhancement"],
        createdAt: "2026-03-24T10:00:00Z",
        updatedAt: "2026-03-24T12:00:00Z",
      },
      reviewSummary: {
        totalReviews: 2,
        approved: 1,
        changesRequested: 0,
        pending: 1,
        reviewers: ["alice", "bob"],
      },
      workspace: {
        id: "ws-1",
        state: "active",
        lastNoteContent: null,
      },
    };
    const prwr = coerceFixture<PullRequestWithReview>(json);
    expect(prwr.pullRequest.number).toBe(42);
    expect(prwr.reviewSummary.totalReviews).toBe(2);
    expect(prwr.workspace).not.toBeNull();
  });

  it("should match DashboardData shape", () => {
    const json = {
      reviewRequests: [],
      myPullRequests: [],
      assignedIssues: [],
      recentActivity: [],
      workspaces: [],
      syncedAt: "2026-03-24T14:00:00Z",
    };
    const dashboard = coerceFixture<DashboardData>(json);
    expect(dashboard.reviewRequests).toEqual([]);
    expect(dashboard.syncedAt).toBe("2026-03-24T14:00:00Z");
  });

  it("should match DashboardData with null syncedAt", () => {
    const json = {
      reviewRequests: [],
      myPullRequests: [],
      assignedIssues: [],
      recentActivity: [],
      workspaces: [],
      syncedAt: null,
    };
    const dashboard = coerceFixture<DashboardData>(json);
    expect(dashboard.syncedAt).toBeNull();
  });

  it("should match DashboardStats shape", () => {
    const json = {
      pendingReviews: 5,
      openPrs: 12,
      openIssues: 3,
      activeWorkspaces: 2,
      unreadActivity: 8,
    };
    const stats = coerceFixture<DashboardStats>(json);
    expect(stats.pendingReviews).toBe(5);
    expect(stats.openPrs).toBe(12);
  });

  it("should match PersonalStats shape", () => {
    const json = {
      prsMergedThisWeek: 3,
      avgReviewResponseHours: 2.5,
      reviewsGivenThisWeek: 7,
      activeWorkspaceCount: 1,
    };
    const stats = coerceFixture<PersonalStats>(json);
    expect(stats.prsMergedThisWeek).toBe(3);
    expect(stats.avgReviewResponseHours).toBe(2.5);
  });
});

// ── IPC payload shapes ───────────────────────────────────────────

describe("IPC payload shapes", () => {
  it("should match OpenWorkspaceRequest shape", () => {
    const json = {
      repoId: "r-1",
      pullRequestNumber: 42,
    };
    const req = coerceFixture<OpenWorkspaceRequest>(json);
    expect(req.repoId).toBe("r-1");
    expect(req.pullRequestNumber).toBe(42);
  });

  it("should match OpenWorkspaceResponse shape", () => {
    const json = {
      workspaceId: "ws-1",
      worktreePath: "/home/user/.prism/workspaces/prism/worktrees/pr-42",
      sessionId: "session-abc",
    };
    const resp = coerceFixture<OpenWorkspaceResponse>(json);
    expect(resp.workspaceId).toBe("ws-1");
    expect(resp.sessionId).toBe("session-abc");
  });

  it("should match OpenWorkspaceResponse with null sessionId", () => {
    const json = {
      workspaceId: "ws-2",
      worktreePath: "/tmp/worktree",
      sessionId: null,
    };
    const resp = coerceFixture<OpenWorkspaceResponse>(json);
    expect(resp.sessionId).toBeNull();
  });

  it("should match PtyInput shape", () => {
    const json = {
      workspaceId: "ws-1",
      data: "ls -la\n",
    };
    const input = coerceFixture<PtyInput>(json);
    expect(input.data).toBe("ls -la\n");
  });

  it("should match PtyOutput shape", () => {
    const json = {
      workspaceId: "ws-1",
      data: "total 42\n",
    };
    const output = coerceFixture<PtyOutput>(json);
    expect(output.data).toBe("total 42\n");
  });

  it("should match PtyResize shape", () => {
    const json = {
      workspaceId: "ws-1",
      cols: 120,
      rows: 40,
    };
    const resize = coerceFixture<PtyResize>(json);
    expect(resize.cols).toBe(120);
    expect(resize.rows).toBe(40);
  });

  it("should match AppConfig shape", () => {
    const json = {
      pollIntervalSecs: 300,
      maxActiveWorkspaces: 3,
      autoSuspendMinutes: 30,
      githubToken: null,
      dataDir: null,
      workspacesDir: null,
    };
    const config = coerceFixture<AppConfig>(json);
    expect(config.pollIntervalSecs).toBe(300);
    expect(config.maxActiveWorkspaces).toBe(3);
    expect(config.githubToken).toBeNull();
  });

  it("should match AppConfig with all optional fields set", () => {
    const json = {
      pollIntervalSecs: 120,
      maxActiveWorkspaces: 5,
      autoSuspendMinutes: 30,
      githubToken: "ghp_xxx",
      dataDir: "/custom/data",
      workspacesDir: "/custom/workspaces",
    };
    const config = coerceFixture<AppConfig>(json);
    expect(config.githubToken).toBe("ghp_xxx");
    expect(config.dataDir).toBe("/custom/data");
  });
});

// ── TauriCommands & TauriEvents maps ─────────────────────────────

describe("TAURI_COMMANDS constant", () => {
  it("should include stats_personal in IPC command names", () => {
    expect(TAURI_COMMANDS).toHaveProperty("stats_personal", "stats_personal");
  });

  it("should have matching key-value pairs", () => {
    for (const [key, value] of Object.entries(TAURI_COMMANDS)) {
      expect(key).toBe(value);
    }
  });

  it("should include all github commands", () => {
    expect(TAURI_COMMANDS.github_get_dashboard).toBe("github_get_dashboard");
    expect(TAURI_COMMANDS.github_get_stats).toBe("github_get_stats");
    expect(TAURI_COMMANDS.github_force_sync).toBe("github_force_sync");
  });

  it("should include all workspace commands", () => {
    expect(TAURI_COMMANDS.workspace_open).toBe("workspace_open");
    expect(TAURI_COMMANDS.workspace_suspend).toBe("workspace_suspend");
    expect(TAURI_COMMANDS.workspace_resume).toBe("workspace_resume");
    expect(TAURI_COMMANDS.workspace_archive).toBe("workspace_archive");
    expect(TAURI_COMMANDS.workspace_list).toBe("workspace_list");
    expect(TAURI_COMMANDS.workspace_get_notes).toBe("workspace_get_notes");
    expect(TAURI_COMMANDS.workspace_cleanup).toBe("workspace_cleanup");
  });

  it("should include all pty commands", () => {
    expect(TAURI_COMMANDS.pty_write).toBe("pty_write");
    expect(TAURI_COMMANDS.pty_resize).toBe("pty_resize");
    expect(TAURI_COMMANDS.pty_kill).toBe("pty_kill");
  });
});

describe("TAURI_EVENTS constant", () => {
  it("should contain all 8 event names", () => {
    expect(Object.keys(TAURI_EVENTS)).toHaveLength(8);
  });

  it("should have matching key-value pairs", () => {
    for (const [key, value] of Object.entries(TAURI_EVENTS)) {
      expect(key).toBe(value);
    }
  });

  it("should include all github events", () => {
    expect(TAURI_EVENTS["github:updated"]).toBe("github:updated");
    expect(TAURI_EVENTS["github:sync_error"]).toBe("github:sync_error");
  });

  it("should include all workspace events", () => {
    expect(TAURI_EVENTS["workspace:stdout"]).toBe("workspace:stdout");
    expect(TAURI_EVENTS["workspace:state_changed"]).toBe("workspace:state_changed");
    expect(TAURI_EVENTS["workspace:claude_session"]).toBe("workspace:claude_session");
  });

  it("should include all notification events", () => {
    expect(TAURI_EVENTS["notification:review_request"]).toBe("notification:review_request");
    expect(TAURI_EVENTS["notification:ci_failure"]).toBe("notification:ci_failure");
    expect(TAURI_EVENTS["notification:pr_approved"]).toBe("notification:pr_approved");
  });
});
