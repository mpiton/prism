import { invoke } from "@tauri-apps/api/core";
import type { AuthStatus } from "./types";

export async function authSetToken(token: string): Promise<string> {
  return invoke<string>("auth_set_token", { token });
}

export async function authGetStatus(): Promise<AuthStatus> {
  return invoke<AuthStatus>("auth_get_status");
}

export async function authLogout(): Promise<void> {
  return invoke<void>("auth_logout");
}
