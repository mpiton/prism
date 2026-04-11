// ── Tauri IPC command & event registries ──

export type TauriCommandName =
  | "github_get_dashboard"
  | "github_get_stats"
  | "github_force_sync"
  | "repos_list"
  | "repos_set_enabled"
  | "repos_set_local_path"
  | "workspace_open"
  | "workspace_suspend"
  | "workspace_resume"
  | "workspace_archive"
  | "workspace_list"
  | "workspace_get_notes"
  | "workspace_cleanup"
  | "pty_write"
  | "pty_resize"
  | "pty_kill"
  | "config_get"
  | "config_set"
  | "activity_mark_read"
  | "activity_mark_all_read"
  | "stats_personal"
  | "auth_set_token"
  | "auth_get_status"
  | "auth_logout"
  | "workspace_list_enriched"
  | "debug_memory_usage";

export const TAURI_COMMANDS = {
  github_get_dashboard: "github_get_dashboard",
  github_get_stats: "github_get_stats",
  github_force_sync: "github_force_sync",
  repos_list: "repos_list",
  repos_set_enabled: "repos_set_enabled",
  repos_set_local_path: "repos_set_local_path",
  workspace_open: "workspace_open",
  workspace_suspend: "workspace_suspend",
  workspace_resume: "workspace_resume",
  workspace_archive: "workspace_archive",
  workspace_list: "workspace_list",
  workspace_get_notes: "workspace_get_notes",
  workspace_cleanup: "workspace_cleanup",
  workspace_list_enriched: "workspace_list_enriched",
  pty_write: "pty_write",
  pty_resize: "pty_resize",
  pty_kill: "pty_kill",
  config_get: "config_get",
  config_set: "config_set",
  activity_mark_read: "activity_mark_read",
  activity_mark_all_read: "activity_mark_all_read",
  stats_personal: "stats_personal",
  auth_set_token: "auth_set_token",
  auth_get_status: "auth_get_status",
  auth_logout: "auth_logout",
  debug_memory_usage: "debug_memory_usage",
} as const satisfies Record<TauriCommandName, TauriCommandName>;

export type TauriEventName =
  | "github:updated"
  | "github:sync_error"
  | "auth:expired"
  | "auth:restored"
  | "workspace:stdout"
  | "workspace:state_changed"
  | "workspace:claude_session"
  | "notification:review_request"
  | "notification:ci_failure"
  | "notification:pr_approved";

export const TAURI_EVENTS = {
  "github:updated": "github:updated",
  "github:sync_error": "github:sync_error",
  "auth:expired": "auth:expired",
  "auth:restored": "auth:restored",
  "workspace:stdout": "workspace:stdout",
  "workspace:state_changed": "workspace:state_changed",
  "workspace:claude_session": "workspace:claude_session",
  "notification:review_request": "notification:review_request",
  "notification:ci_failure": "notification:ci_failure",
  "notification:pr_approved": "notification:pr_approved",
} as const satisfies Record<TauriEventName, TauriEventName>;
