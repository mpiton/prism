import type { MouseEvent, ReactElement } from "react";
import { timeAgo } from "../../lib/timeAgo";
import type { Issue, IssueState } from "../../lib/types";
import { LabelTag } from "../atoms/LabelTag";

interface IssueCardProps {
  readonly issue: Issue;
  readonly onOpen: (url: string) => void;
}

const STATE_DOT_COLOR: Record<IssueState, string> = {
  open: "bg-green",
  closed: "bg-purple",
};

export function IssueCard({ issue, onOpen }: IssueCardProps): ReactElement {
  function handleClick(e: MouseEvent) {
    e.preventDefault();
    onOpen(issue.url);
  }

  return (
    <div
      data-testid="issue-card"
      className="flex items-center gap-3 rounded border border-border px-3 py-2 hover:bg-surface-hover"
    >
      <a
        href={issue.url}
        onClick={handleClick}
        className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 no-underline"
      >
        <span
          data-testid="issue-state-dot"
          className={`h-2.5 w-2.5 shrink-0 rounded-full ${STATE_DOT_COLOR[issue.state]}`}
        />

        <span className="min-w-0 truncate text-sm font-medium text-foreground">
          {issue.title}
        </span>

        <span className="shrink-0 text-xs text-dim">#{issue.number}</span>

        <span className="shrink-0 text-xs text-dim">{issue.repoId}</span>

        {issue.labels.length > 0 && (
          <span className="flex items-center gap-1">
            {issue.labels.map((label, idx) => (
              <LabelTag key={`${label}-${idx}`} name={label} />
            ))}
          </span>
        )}

        <span data-testid="time-ago" className="shrink-0 text-xs text-dim">
          {timeAgo(issue.updatedAt)}
        </span>
      </a>
    </div>
  );
}
