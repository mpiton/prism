import { describe, it, expect, vi, beforeEach } from "vitest";
import { TAURI_COMMANDS } from "./types";
import type { TauriEventName } from "./types";

// ── Mocks ────────────────────────────────────────────────────────

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const mockListen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

beforeEach(() => {
  mockInvoke.mockReset();
  mockListen.mockReset();
});

// ── Export completeness ──────────────────────────────────────────

describe("IPC wrapper exports", () => {
  it("should export all github methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.getGithubDashboard).toBe("function");
    expect(typeof mod.getGithubStats).toBe("function");
    expect(typeof mod.forceGithubSync).toBe("function");
  });

  it("should export all repos methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.listRepos).toBe("function");
    expect(typeof mod.setRepoEnabled).toBe("function");
    expect(typeof mod.setRepoLocalPath).toBe("function");
  });

  it("should export all workspace methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.openWorkspace).toBe("function");
    expect(typeof mod.suspendWorkspace).toBe("function");
    expect(typeof mod.resumeWorkspace).toBe("function");
    expect(typeof mod.archiveWorkspace).toBe("function");
    expect(typeof mod.listWorkspaces).toBe("function");
    expect(typeof mod.getWorkspaceNotes).toBe("function");
    expect(typeof mod.cleanupWorkspaces).toBe("function");
  });

  it("should export all pty methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.ptyWrite).toBe("function");
    expect(typeof mod.ptyResize).toBe("function");
    expect(typeof mod.ptyKill).toBe("function");
  });

  it("should export all config methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.getConfig).toBe("function");
    expect(typeof mod.setConfig).toBe("function");
  });

  it("should export all activity methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.markActivityRead).toBe("function");
    expect(typeof mod.markAllActivityRead).toBe("function");
  });

  it("should export all auth methods", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.authSetToken).toBe("function");
    expect(typeof mod.authGetStatus).toBe("function");
    expect(typeof mod.authLogout).toBe("function");
  });

  it("should export onEvent listener", async () => {
    const mod = await import("./tauri");
    expect(typeof mod.onEvent).toBe("function");
  });

  it("should have a wrapper for every TAURI_COMMANDS entry", async () => {
    const mod = await import("./tauri");
    const exportedFns = Object.values(mod).filter(
      (v) => typeof v === "function",
    );
    // 23 commands + 1 onEvent = 24 functions
    expect(exportedFns.length).toBe(24);
  });
});

// ── Invoke correctness ──────────────────────────────────────────

describe("GitHub wrappers", () => {
  it("should invoke github_get_dashboard", async () => {
    const { getGithubDashboard } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await getGithubDashboard();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.github_get_dashboard);
  });

  it("should invoke github_get_stats", async () => {
    const { getGithubStats } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await getGithubStats();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.github_get_stats);
  });

  it("should invoke github_force_sync", async () => {
    const { forceGithubSync } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await forceGithubSync();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.github_force_sync);
  });
});

describe("Repos wrappers", () => {
  it("should invoke repos_list", async () => {
    const { listRepos } = await import("./tauri");
    mockInvoke.mockResolvedValue([]);
    await listRepos();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.repos_list);
  });

  it("should invoke repos_set_enabled with params", async () => {
    const { setRepoEnabled } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await setRepoEnabled("r-1", true);
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.repos_set_enabled, {
      repoId: "r-1",
      enabled: true,
    });
  });

  it("should invoke repos_set_local_path with params", async () => {
    const { setRepoLocalPath } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await setRepoLocalPath("r-1", "/path/to/repo");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.repos_set_local_path, {
      repoId: "r-1",
      path: "/path/to/repo",
    });
  });
});

describe("Workspace wrappers", () => {
  it("should invoke workspace_open with request payload", async () => {
    const { openWorkspace } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await openWorkspace({ repoId: "r-1", pullRequestNumber: 42 });
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_open, {
      request: { repoId: "r-1", pullRequestNumber: 42 },
    });
  });

  it("should invoke workspace_suspend with workspaceId", async () => {
    const { suspendWorkspace } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await suspendWorkspace("ws-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_suspend, {
      workspaceId: "ws-1",
    });
  });

  it("should invoke workspace_resume with workspaceId", async () => {
    const { resumeWorkspace } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await resumeWorkspace("ws-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_resume, {
      workspaceId: "ws-1",
    });
  });

  it("should invoke workspace_archive with workspaceId", async () => {
    const { archiveWorkspace } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await archiveWorkspace("ws-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_archive, {
      workspaceId: "ws-1",
    });
  });

  it("should invoke workspace_list", async () => {
    const { listWorkspaces } = await import("./tauri");
    mockInvoke.mockResolvedValue([]);
    await listWorkspaces();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_list);
  });

  it("should invoke workspace_get_notes with workspaceId", async () => {
    const { getWorkspaceNotes } = await import("./tauri");
    mockInvoke.mockResolvedValue([]);
    await getWorkspaceNotes("ws-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_get_notes, {
      workspaceId: "ws-1",
    });
  });

  it("should invoke workspace_cleanup", async () => {
    const { cleanupWorkspaces } = await import("./tauri");
    mockInvoke.mockResolvedValue(3);
    const result = await cleanupWorkspaces();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.workspace_cleanup);
    expect(result).toBe(3);
  });
});

describe("PTY wrappers", () => {
  it("should invoke pty_write with input payload", async () => {
    const { ptyWrite } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await ptyWrite({ workspaceId: "ws-1", data: "ls\n" });
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.pty_write, {
      input: { workspaceId: "ws-1", data: "ls\n" },
    });
  });

  it("should invoke pty_resize with resize payload", async () => {
    const { ptyResize } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await ptyResize({ workspaceId: "ws-1", cols: 120, rows: 40 });
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.pty_resize, {
      resize: { workspaceId: "ws-1", cols: 120, rows: 40 },
    });
  });

  it("should invoke pty_kill with workspaceId", async () => {
    const { ptyKill } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await ptyKill("ws-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.pty_kill, {
      workspaceId: "ws-1",
    });
  });
});

describe("Config wrappers", () => {
  it("should invoke config_get", async () => {
    const { getConfig } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await getConfig();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.config_get);
  });

  it("should invoke config_set with partial config", async () => {
    const { setConfig } = await import("./tauri");
    mockInvoke.mockResolvedValue({});
    await setConfig({ pollIntervalSecs: 120 });
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.config_set, {
      partial: { pollIntervalSecs: 120 },
    });
  });
});

describe("Activity wrappers", () => {
  it("should invoke activity_mark_read with activityId", async () => {
    const { markActivityRead } = await import("./tauri");
    mockInvoke.mockResolvedValue(true);
    const result = await markActivityRead("a-1");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.activity_mark_read, {
      activityId: "a-1",
    });
    expect(result).toBe(true);
  });

  it("should invoke activity_mark_all_read", async () => {
    const { markAllActivityRead } = await import("./tauri");
    mockInvoke.mockResolvedValue(5);
    const result = await markAllActivityRead();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.activity_mark_all_read);
    expect(result).toBe(5);
  });
});

describe("Auth wrappers", () => {
  it("should invoke auth_set_token with token", async () => {
    const { authSetToken } = await import("./tauri");
    mockInvoke.mockResolvedValue("mpiton");
    const result = await authSetToken("ghp_xxx");
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.auth_set_token, {
      token: "ghp_xxx",
    });
    expect(result).toBe("mpiton");
  });

  it("should invoke auth_get_status", async () => {
    const { authGetStatus } = await import("./tauri");
    mockInvoke.mockResolvedValue({ connected: true, username: "mpiton", error: null });
    await authGetStatus();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.auth_get_status);
  });

  it("should invoke auth_logout", async () => {
    const { authLogout } = await import("./tauri");
    mockInvoke.mockResolvedValue(undefined);
    await authLogout();
    expect(mockInvoke).toHaveBeenCalledWith(TAURI_COMMANDS.auth_logout);
  });
});

// ── onEvent ─────────────────────────────────────────────────────

describe("onEvent listener", () => {
  it("should call listen with the correct event name", async () => {
    const { onEvent } = await import("./tauri");
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);
    const handler = vi.fn();

    await onEvent("github:updated" as TauriEventName, handler);
    expect(mockListen).toHaveBeenCalledWith("github:updated", expect.any(Function));
  });

  it("should return the unlisten function from listen", async () => {
    const { onEvent } = await import("./tauri");
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);

    const result = await onEvent("workspace:stdout" as TauriEventName, vi.fn());
    expect(result).toBe(unlisten);
  });

  it("should forward event payload to handler", async () => {
    const { onEvent } = await import("./tauri");
    mockListen.mockImplementation((_event: string, cb: (e: unknown) => void) => {
      cb({ event: "github:updated", id: 1, payload: { syncedAt: "2026-03-27" } });
      return Promise.resolve(vi.fn());
    });

    const handler = vi.fn();
    await onEvent("github:updated" as TauriEventName, handler);
    expect(handler).toHaveBeenCalledWith({ syncedAt: "2026-03-27" });
  });
});
