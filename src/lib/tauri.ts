import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { TAURI_COMMANDS } from "./types";
import type {
  AppConfig,
  AuthStatus,
  DashboardData,
  DashboardStats,
  OpenWorkspaceRequest,
  OpenWorkspaceResponse,
  PartialAppConfig,
  PtyInput,
  PtyResize,
  Repo,
  TauriEventName,
  Workspace,
  WorkspaceNote,
} from "./types";

// ── Auth ─────────────────────────────────────────────────────────

export async function authSetToken(token: string): Promise<string> {
  return invoke<string>(TAURI_COMMANDS.auth_set_token, { token });
}

export async function authGetStatus(): Promise<AuthStatus> {
  return invoke<AuthStatus>(TAURI_COMMANDS.auth_get_status);
}

export async function authLogout(): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.auth_logout);
}

// ── GitHub ───────────────────────────────────────────────────────

export async function getGithubDashboard(): Promise<DashboardData> {
  return invoke<DashboardData>(TAURI_COMMANDS.github_get_dashboard);
}

export async function getGithubStats(): Promise<DashboardStats> {
  return invoke<DashboardStats>(TAURI_COMMANDS.github_get_stats);
}

export async function forceGithubSync(): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.github_force_sync);
}

// ── Repos ────────────────────────────────────────────────────────

export async function listRepos(): Promise<Repo[]> {
  return invoke<Repo[]>(TAURI_COMMANDS.repos_list);
}

export async function setRepoEnabled(repoId: string, enabled: boolean): Promise<Repo> {
  return invoke<Repo>(TAURI_COMMANDS.repos_set_enabled, { repoId, enabled });
}

export async function setRepoLocalPath(repoId: string, path: string | null): Promise<Repo> {
  return invoke<Repo>(TAURI_COMMANDS.repos_set_local_path, { repoId, path });
}

// ── Workspaces ───────────────────────────────────────────────────

export async function openWorkspace(request: OpenWorkspaceRequest): Promise<OpenWorkspaceResponse> {
  return invoke<OpenWorkspaceResponse>(TAURI_COMMANDS.workspace_open, { request });
}

export async function suspendWorkspace(workspaceId: string): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.workspace_suspend, { workspaceId });
}

export async function resumeWorkspace(workspaceId: string): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.workspace_resume, { workspaceId });
}

export async function archiveWorkspace(workspaceId: string): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.workspace_archive, { workspaceId });
}

export async function listWorkspaces(): Promise<Workspace[]> {
  return invoke<Workspace[]>(TAURI_COMMANDS.workspace_list);
}

export async function getWorkspaceNotes(workspaceId: string): Promise<WorkspaceNote[]> {
  return invoke<WorkspaceNote[]>(TAURI_COMMANDS.workspace_get_notes, { workspaceId });
}

export async function cleanupWorkspaces(): Promise<number> {
  return invoke<number>(TAURI_COMMANDS.workspace_cleanup);
}

// ── PTY ──────────────────────────────────────────────────────────

export async function ptyWrite(input: PtyInput): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.pty_write, { input });
}

export async function ptyResize(resize: PtyResize): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.pty_resize, { resize });
}

export async function ptyKill(workspaceId: string): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.pty_kill, { workspaceId });
}

// ── Config ───────────────────────────────────────────────────────

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>(TAURI_COMMANDS.config_get);
}

export async function setConfig(partial: PartialAppConfig): Promise<AppConfig> {
  return invoke<AppConfig>(TAURI_COMMANDS.config_set, { partial });
}

// ── Activity ─────────────────────────────────────────────────────

export async function markActivityRead(activityId: string): Promise<boolean> {
  return invoke<boolean>(TAURI_COMMANDS.activity_mark_read, { activityId });
}

export async function markAllActivityRead(): Promise<number> {
  return invoke<number>(TAURI_COMMANDS.activity_mark_all_read);
}

// ── Events ───────────────────────────────────────────────────────

export async function onEvent<T = unknown>(
  event: TauriEventName,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
