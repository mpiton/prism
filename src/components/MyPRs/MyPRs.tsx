import { type ReactElement, useMemo, useState } from "react";
import type { PullRequestWithReview } from "../../lib/types";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { MyPrCard } from "./MyPrCard";

interface MyPRsProps {
  readonly prs: readonly PullRequestWithReview[];
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (workspaceId: string) => void;
}

type Tab = "open" | "merged";

function isOpen(pr: PullRequestWithReview): boolean {
  const { state } = pr.pullRequest;
  return state === "open" || state === "draft";
}

function isMerged(pr: PullRequestWithReview): boolean {
  return pr.pullRequest.state === "merged";
}

export function MyPRs({
  prs,
  onOpen,
  onWorkspaceAction,
}: MyPRsProps): ReactElement {
  const [tab, setTab] = useState<Tab>("open");

  const openPrs = prs.filter(isOpen);
  const mergedPrs = prs.filter(isMerged);
  const visible = tab === "open" ? openPrs : mergedPrs;

  const navItems = useMemo(
    () =>
      prs
        .filter(tab === "open" ? isOpen : isMerged)
        .map((pr) => ({ url: pr.pullRequest.url })),
    [prs, tab],
  );
  useRegisterNavigableItems(navItems);

  return (
    <section data-testid="my-prs" className="flex flex-col gap-2">
      <SectionHead title="My PRs" count={openPrs.length + mergedPrs.length} />

      <div className="flex gap-1" role="group" aria-label="Filter by state">
        <button
          type="button"
          aria-pressed={tab === "open"}
          onClick={() => setTab("open")}
          className={`rounded px-2 py-0.5 text-xs transition-colors ${
            tab === "open"
              ? "bg-accent text-bg font-semibold"
              : "text-dim hover:text-foreground"
          }`}
        >
          Open {openPrs.length}
        </button>
        <button
          type="button"
          aria-pressed={tab === "merged"}
          onClick={() => setTab("merged")}
          className={`rounded px-2 py-0.5 text-xs transition-colors ${
            tab === "merged"
              ? "bg-accent text-bg font-semibold"
              : "text-dim hover:text-foreground"
          }`}
        >
          Merged {mergedPrs.length}
        </button>
      </div>

      {visible.length === 0 ? (
        <EmptyState icon="↗" message="No pull requests to display" />
      ) : (
        <div className="flex flex-col gap-1">
          {visible.map((pr) => (
            <MyPrCard
              key={pr.pullRequest.id}
              data={pr}
              onOpen={onOpen}
              onWorkspaceAction={onWorkspaceAction}
            />
          ))}
        </div>
      )}
    </section>
  );
}
