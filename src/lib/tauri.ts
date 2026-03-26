import { invoke } from "@tauri-apps/api/core";
import { TAURI_COMMANDS } from "./types";
import type { AuthStatus, DashboardData } from "./types";

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
