// ── Application configuration payloads ──

export interface AppConfig {
  /** Rust type is u64 — safe as `number` since practical values never exceed 2^53. */
  readonly pollIntervalSecs: number;
  readonly maxActiveWorkspaces: number;
  /** Minutes of inactivity before an active workspace is auto-suspended (min 5, default 30). */
  readonly autoSuspendMinutes: number;
  readonly githubToken: string | null;
  readonly dataDir: string | null;
  readonly workspacesDir: string | null;
  readonly claudeAuthMode: "oauth" | "api_key";
  readonly claudeAutoGenerateMd: boolean;
}

/** Partial update payload for `config_set`. Mirrors Rust `PartialAppConfig`.
 *
 * - `undefined` (key absent) = leave unchanged
 * - `null` (for nullable fields) = explicitly clear to null
 * - value = set to that value
 */
export interface PartialAppConfig {
  readonly pollIntervalSecs?: number;
  readonly maxActiveWorkspaces?: number;
  readonly autoSuspendMinutes?: number;
  /** undefined = leave unchanged, null = clear token, string = set token.
   * Prefer `authSetToken` for validated token updates. */
  readonly githubToken?: string | null;
  readonly dataDir?: string | null;
  readonly workspacesDir?: string | null;
  readonly claudeAuthMode?: "oauth" | "api_key";
  readonly claudeAutoGenerateMd?: boolean;
}
