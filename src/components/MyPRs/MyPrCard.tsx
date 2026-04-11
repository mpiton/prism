import { type ReactElement, useState } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { timeAgo } from "../../lib/timeAgo";
import type { CiStatus, PullRequestWithReview } from "../../lib/types";
import { CI } from "../atoms/CI";
import { Diff } from "../atoms/Diff";
import { WsBadge } from "../atoms/WsBadge";

interface MyPrCardProps {
  readonly data: PullRequestWithReview;
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (params: {
    readonly repoId: string;
    readonly pullRequestNumber: number;
    readonly headRefName: string;
    readonly workspaceId?: string;
    readonly workspaceState?: string;
  }) => void;
}

function isMergeable(data: PullRequestWithReview): boolean {
  const { pullRequest: pr, reviewSummary } = data;
  return (
    pr.state === "open" &&
    pr.ciStatus === "success" &&
    reviewSummary.approved > 0 &&
    reviewSummary.changesRequested === 0
  );
}

const CI_DOT_COLOR: Record<CiStatus, string> = {
  success: "bg-green",
  failure: "bg-red",
  running: "bg-orange",
  pending: "bg-dim",
  cancelled: "bg-dim",
};

interface ReviewDot {
  readonly key: string;
  readonly color: string;
}

function buildReviewDots(
  reviewSummary: PullRequestWithReview["reviewSummary"],
): readonly ReviewDot[] {
  const dots: ReviewDot[] = [];
  for (let i = 0; i < reviewSummary.approved; i++)
    dots.push({ key: `approved-${i}`, color: "bg-green" });
  for (let i = 0; i < reviewSummary.changesRequested; i++)
    dots.push({ key: `changes-${i}`, color: "bg-red" });
  for (let i = 0; i < reviewSummary.pending; i++)
    dots.push({ key: `pending-${i}`, color: "bg-dim" });
  return dots;
}

export function MyPrCard({ data, onOpen, onWorkspaceAction }: MyPrCardProps): ReactElement {
  const { pullRequest: pr, workspace } = data;
  const merged = pr.state === "merged";
  const [loading, setLoading] = useState(false);

  function handleWorkspace() {
    if (!onWorkspaceAction || loading) return;
    setLoading(true);
    Promise.resolve(
      onWorkspaceAction({
        repoId: pr.repoId,
        pullRequestNumber: pr.number,
        headRefName: pr.headRefName,
        workspaceId: workspace?.id,
        workspaceState: workspace?.state,
      }),
    ).finally(() => setLoading(false));
  }
  const reviewDots = buildReviewDots(data.reviewSummary);

  function handleClick(e: React.MouseEvent) {
    e.preventDefault();
    onOpen(pr.url);
  }

  return (
    <div
      data-testid="my-pr-card"
      className={`flex items-center gap-3 rounded border border-border px-3 py-2 hover:bg-surface-hover${merged ? " opacity-50" : ""}`}
    >
      <a
        href={pr.url}
        onClick={handleClick}
        aria-label={`PR #${pr.number}: ${pr.title}`}
        className={`${FOCUS_RING} flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded no-underline`}
      >
        <span
          data-testid="ci-dot"
          aria-hidden="true"
          className={`h-2.5 w-2.5 shrink-0 rounded-full ${CI_DOT_COLOR[pr.ciStatus]}`}
        />

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2">
            <span
              className={`min-w-0 truncate text-sm font-medium text-foreground${merged ? " line-through" : ""}`}
              title={pr.title}
            >
              {pr.title}
            </span>
            <span className="shrink-0 text-xs text-dim">#{pr.number}</span>
            {isMergeable(data) && (
              <span className="shrink-0 rounded bg-green/20 px-1.5 py-0.5 text-xs font-semibold text-green">
                MERGEABLE
              </span>
            )}
          </div>

          <div className="flex items-center gap-2 text-xs text-dim">
            {pr.additions !== undefined && pr.deletions !== undefined && (
              <Diff additions={pr.additions} deletions={pr.deletions} />
            )}
            <CI status={pr.ciStatus} />
            <span className="flex items-center gap-0.5">
              {reviewDots.map((dot) => (
                <span
                  key={dot.key}
                  data-testid="review-dot"
                  aria-hidden="true"
                  className={`h-2 w-2 rounded-full ${dot.color}`}
                />
              ))}
            </span>
            <span data-testid="time-ago" className="ml-auto shrink-0">
              {timeAgo(pr.updatedAt)}
            </span>
          </div>
        </div>
      </a>

      {onWorkspaceAction &&
        (pr.state === "open" || pr.state === "draft") &&
        ((workspace && workspace.state !== "archived") || pr.headRefName) && (
          <WsBadge
            state={workspace?.state === "archived" ? undefined : workspace?.state}
            loading={loading}
            onClick={handleWorkspace}
            ariaLabel={`${workspace?.state === "active" ? "Resume" : workspace?.state === "suspended" ? "Wake" : "Open"} workspace for PR #${pr.number}`}
          />
        )}
    </div>
  );
}
