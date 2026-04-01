import type { ReactElement } from "react";
import type { CiStatus } from "../../lib/types";
import { ptyWrite } from "../../lib/tauri";
import { CI } from "../atoms/CI";

interface WorkspaceStatusBarProps {
  readonly workspaceId: string;
  readonly branch: string;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus | null;
  readonly sessionName: string | null;
  readonly sessionCount: number;
  readonly githubUrl: string;
  readonly disabled?: boolean;
}

export function WorkspaceStatusBar({
  workspaceId,
  branch,
  ahead,
  behind,
  ciStatus,
  sessionName,
  sessionCount,
  githubUrl,
  disabled = false,
}: WorkspaceStatusBarProps): ReactElement {
  const hasSync = ahead > 0 || behind > 0;
  const safeGithubUrl = githubUrl.startsWith("https://") ? githubUrl : "#";

  function handlePtyCommand(command: string): void {
    if (disabled) return;
    ptyWrite({ workspaceId, data: `${command}\n` }).catch((err: unknown) => {
      console.error("[WorkspaceStatusBar] ptyWrite failed:", err);
    });
  }

  return (
    <div
      data-testid="workspace-statusbar"
      className="flex items-center gap-3 border-t border-border bg-surface px-3 py-1.5 text-xs"
    >
      {/* Informational live region */}
      <span role="status" aria-label="Workspace status" className="contents">
        <span data-testid="status-branch" className="font-mono text-accent">
          {branch}
        </span>

        {hasSync && (
          <span className="flex gap-1 font-mono text-dim">
            {ahead > 0 && <span data-testid="status-ahead">↑{ahead}</span>}
            {behind > 0 && <span data-testid="status-behind">↓{behind}</span>}
          </span>
        )}

        {ciStatus !== null && (
          <span data-testid="status-ci">
            <CI status={ciStatus} />
          </span>
        )}

        {sessionName !== null && (
          <>
            <span data-testid="status-session" className="text-purple">
              {sessionName}
            </span>
            <span
              data-testid="status-session-count"
              className="text-dim"
              aria-label={`${sessionCount} sessions`}
            >
              {sessionCount}
            </span>
          </>
        )}
      </span>

      <span className="flex-1" />

      {/* Action buttons */}
      <div role="toolbar" aria-label="Workspace actions" className="flex gap-1">
        <button
          data-testid="btn-git-push"
          type="button"
          aria-label="Git push"
          disabled={disabled}
          className="rounded px-2 py-0.5 text-muted hover:bg-surface-hover hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          onClick={() => handlePtyCommand("git push")}
        >
          push
        </button>
        <button
          data-testid="btn-git-pull"
          type="button"
          aria-label="Git pull"
          disabled={disabled}
          className="rounded px-2 py-0.5 text-muted hover:bg-surface-hover hover:text-text disabled:cursor-not-allowed disabled:opacity-40"
          onClick={() => handlePtyCommand("git pull")}
        >
          pull
        </button>
        <a
          data-testid="btn-open-github"
          href={safeGithubUrl}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Open pull request on GitHub"
          aria-disabled={disabled || undefined}
          tabIndex={disabled ? -1 : undefined}
          onClick={disabled ? (e) => e.preventDefault() : undefined}
          className={`rounded px-2 py-0.5 text-muted hover:bg-surface-hover hover:text-text ${disabled ? "pointer-events-none opacity-40" : ""}`}
        >
          github
        </a>
      </div>
    </div>
  );
}
