import { memo, type ReactElement, useCallback } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { timeAgo } from "../../lib/timeAgo";
import { SELECTED_ITEM_CLASS } from "../../lib/uiClasses";
import type { GithubNotification, NotificationSubjectType } from "../../lib/types/github";

interface NotificationCardProps {
  readonly data: GithubNotification;
  readonly onOpen: (url: string) => void;
  readonly isSelected?: boolean;
}

const TYPE_ICON: Record<NotificationSubjectType, string> = {
  pullRequest: "⇄",
  issue: "●",
  release: "↗",
  discussion: "✎",
  checkSuite: "✓",
  commit: "◆",
  other: "•",
};

const TYPE_LABEL: Record<NotificationSubjectType, string> = {
  pullRequest: "Pull request",
  issue: "Issue",
  release: "Release",
  discussion: "Discussion",
  checkSuite: "Check suite",
  commit: "Commit",
  other: "Notification",
};

/**
 * Humanize a GitHub notification reason.
 *
 * Reference: https://docs.github.com/en/rest/activity/notifications?apiVersion=2022-11-28#about-notification-reasons
 * Unknown reasons fall back to a cleaned-up version of the raw value so the UI
 * never displays raw snake_case strings.
 */
function humanizeReason(reason: string): string {
  const known: Record<string, string> = {
    review_requested: "Review requested",
    mention: "Mentioned",
    team_mention: "Team mentioned",
    assign: "Assigned",
    author: "Author",
    subscribed: "Subscribed",
    comment: "Commented",
    state_change: "State changed",
    ci_activity: "CI activity",
    security_alert: "Security alert",
    manual: "Manual",
  };
  if (known[reason]) return known[reason];
  return reason.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase());
}

/// Only allow http(s) URLs to be forwarded to the Tauri opener.
///
/// Defensive guard: the URL ultimately comes from the GitHub API, but we
/// never trust remote strings reaching `tauriOpen` with schemes like
/// `file://`, `javascript:`, or custom protocol handlers.
function isSafeUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" || parsed.protocol === "http:";
  } catch {
    return false;
  }
}

function NotificationCardImpl({
  data,
  onOpen,
  isSelected = false,
}: NotificationCardProps): ReactElement {
  const { url, title, notificationType, unread, reason, repo, updatedAt } = data;

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      if (!isSafeUrl(url)) {
        console.warn("[NotificationCard] refusing unsafe URL", url);
        return;
      }
      onOpen(url);
    },
    [onOpen, url],
  );

  const accessibleLabel = `${unread ? "Unread " : ""}${TYPE_LABEL[notificationType]}: ${title}`;

  return (
    <div
      data-testid="notification-card"
      data-selected={isSelected ? "true" : undefined}
      className={`flex items-center gap-3 rounded border border-border px-3 py-2 hover:bg-surface-hover${
        unread ? "" : " opacity-60"
      }${isSelected ? ` ${SELECTED_ITEM_CLASS}` : ""}`}
    >
      <a
        href={url}
        onClick={handleClick}
        aria-label={accessibleLabel}
        className={`${FOCUS_RING} flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded no-underline`}
      >
        {unread && (
          <span
            data-testid="unread-indicator"
            aria-hidden="true"
            className="h-2 w-2 shrink-0 rounded-full bg-accent"
          />
        )}

        <span
          data-testid="notification-type-icon"
          data-type={notificationType}
          aria-hidden="true"
          className="shrink-0 text-sm text-dim"
          title={TYPE_LABEL[notificationType]}
        >
          {TYPE_ICON[notificationType]}
        </span>

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-sm font-medium text-foreground" title={title}>
              {title}
            </span>
          </div>

          <div className="flex items-center gap-2 text-xs text-dim">
            <span className="truncate">{repo}</span>
            <span className="shrink-0 rounded bg-surface-hover px-1.5 py-0.5 text-[10px]">
              {humanizeReason(reason)}
            </span>
            <span data-testid="time-ago" className="ml-auto shrink-0">
              {timeAgo(updatedAt)}
            </span>
          </div>
        </div>
      </a>
    </div>
  );
}

export const NotificationCard = memo(NotificationCardImpl);
