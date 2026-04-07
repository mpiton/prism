import type { MouseEvent, ReactElement } from "react";
import { timeAgo } from "../../lib/timeAgo";
import type { Issue, IssueState } from "../../lib/types";
import { LabelTag } from "../atoms/LabelTag";

interface IssueCardProps {
  readonly issue: Issue;
  readonly repoName: string;
  readonly onOpen: (url: string) => void;
}

const STATE_DOT_COLOR: Record<IssueState, string> = {
  open: "bg-green",
  closed: "bg-purple",
};

export function IssueCard({ issue, repoName, onOpen }: IssueCardProps): ReactElement {
  function handleClick(e: MouseEvent) {
    e.preventDefault();
    onOpen(issue.url);
  }

  return (
    <a
      data-testid="issue-card"
      href={issue.url}
      onClick={handleClick}
      aria-label={`Issue #${issue.number}: ${issue.title} (${issue.state})`}
      className="flex min-w-0 cursor-pointer flex-col gap-1 rounded border border-border px-3 py-2 no-underline hover:bg-surface-hover"
    >
      <div className="flex min-w-0 items-center gap-2">
        <span
          data-testid="issue-state-dot"
          aria-hidden="true"
          className={`h-2.5 w-2.5 shrink-0 rounded-full ${STATE_DOT_COLOR[issue.state]}`}
        />
        <span className="shrink-0 text-xs text-dim">#{issue.number}</span>
        <span className="min-w-0 truncate text-sm font-medium text-foreground" title={issue.title}>
          {issue.title}
        </span>
      </div>

      <div className="flex min-w-0 items-center gap-2 pl-[18px]">
        <span className="min-w-0 truncate text-xs text-dim" title={repoName}>{repoName}</span>
        {issue.labels.length > 0 && (
          <span className="flex min-w-0 items-center gap-1 overflow-hidden">
            {issue.labels.map((label) => (
              <LabelTag key={`${issue.id}:${label}`} name={label} />
            ))}
          </span>
        )}
        <span data-testid="time-ago" className="ml-auto shrink-0 text-xs text-dim">
          {timeAgo(issue.updatedAt)}
        </span>
      </div>
    </a>
  );
}
