// ── Authentication state (T-027) ──

export interface AuthStatus {
  readonly connected: boolean;
  readonly username: string | null;
  readonly error: string | null;
}
