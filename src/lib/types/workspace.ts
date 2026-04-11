// ── Workspace domain — state, entries, IPC payloads (T-009, T-011, T-076) ──

import type { CiStatus, WorkspaceState } from "./enums";

export interface Workspace {
  readonly id: string;
  readonly repoId: string;
  readonly pullRequestNumber: number;
  readonly state: WorkspaceState;
  readonly worktreePath: string | null;
  readonly sessionId: string | null;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface WorkspaceNote {
  readonly id: string;
  readonly workspaceId: string;
  readonly content: string;
  readonly createdAt: string;
}

export interface WorkspaceStatusInfo {
  readonly branch: string;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus | null;
  readonly sessionName: string | null;
  readonly sessionCount: number;
  readonly githubUrl: string;
}

export interface WorkspaceListEntry {
  readonly workspace: Workspace;
  readonly branch: string | null;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus | null;
  readonly githubUrl: string | null;
  readonly sessionCount: number;
  readonly diskUsageMb: number | null;
  readonly lastNote: string | null;
}

export interface WorkspaceSummary {
  readonly id: string;
  readonly state: WorkspaceState;
  readonly lastNoteContent: string | null;
}

// ── IPC payloads ─────────────────────────────────────────────────

export interface OpenWorkspaceRequest {
  readonly repoId: string;
  readonly pullRequestNumber: number;
  readonly branch: string;
}

export interface OpenWorkspaceResponse {
  readonly workspaceId: string;
  readonly worktreePath: string;
  readonly sessionId: string | null;
}

export interface PtyInput {
  readonly workspaceId: string;
  readonly data: string;
}

export interface PtyOutput {
  readonly workspaceId: string;
  readonly data: string;
}

export interface PtyResize {
  readonly workspaceId: string;
  readonly cols: number;
  readonly rows: number;
}
