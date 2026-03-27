import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS } from "./types";
import type { AuthStatus, DashboardData, DashboardStats, Repo } from "./types";

export async function authSetToken(token: string): Promise<string> {
  return invoke<string>(TAURI_COMMANDS.auth_set_token, { token });
}

export async function authGetStatus(): Promise<AuthStatus> {
  return invoke<AuthStatus>(TAURI_COMMANDS.auth_get_status);
}

export async function authLogout(): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.auth_logout);
}

export async function getGithubDashboard(): Promise<DashboardData> {
  return invoke<DashboardData>(TAURI_COMMANDS.github_get_dashboard);
}

export async function getGithubStats(): Promise<DashboardStats> {
  return invoke<DashboardStats>(TAURI_COMMANDS.github_get_stats);
}

export async function forceGithubSync(): Promise<void> {
  return invoke<void>(TAURI_COMMANDS.github_force_sync);
}

export async function listRepos(): Promise<Repo[]> {
  return invoke<Repo[]>(TAURI_COMMANDS.repos_list);
}

export async function setRepoEnabled(repoId: string, enabled: boolean): Promise<Repo> {
  return invoke<Repo>(TAURI_COMMANDS.repos_set_enabled, { repoId, enabled });
}

export async function setRepoLocalPath(repoId: string, path: string | null): Promise<Repo> {
  return invoke<Repo>(TAURI_COMMANDS.repos_set_local_path, { repoId, path });
}
