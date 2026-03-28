import { type ReactElement, useState } from "react";
import type { Issue } from "../../lib/types";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { IssueCard } from "./IssueCard";

interface IssuesProps {
  readonly issues: readonly Issue[];
  readonly onOpen: (url: string) => void;
}

type Tab = "open" | "closed";

function isOpen(issue: Issue): boolean {
  return issue.state === "open";
}

function isClosed(issue: Issue): boolean {
  return issue.state === "closed";
}

export function Issues({ issues, onOpen }: IssuesProps): ReactElement {
  const [tab, setTab] = useState<Tab>("open");

  const openIssues = issues.filter(isOpen);
  const closedIssues = issues.filter(isClosed);
  const visible = tab === "open" ? openIssues : closedIssues;

  return (
    <section data-testid="issues" className="flex flex-col gap-2">
      <SectionHead title="Issues" count={issues.length} />

      <div className="flex gap-1" role="group" aria-label="Filter by state">
        <button
          type="button"
          aria-pressed={tab === "open"}
          onClick={() => setTab("open")}
          className={`rounded px-2 py-0.5 text-xs ${
            tab === "open"
              ? "bg-accent text-white"
              : "text-dim hover:text-foreground"
          }`}
        >
          Open {openIssues.length}
        </button>
        <button
          type="button"
          aria-pressed={tab === "closed"}
          onClick={() => setTab("closed")}
          className={`rounded px-2 py-0.5 text-xs ${
            tab === "closed"
              ? "bg-accent text-white"
              : "text-dim hover:text-foreground"
          }`}
        >
          Closed {closedIssues.length}
        </button>
      </div>

      {visible.length === 0 ? (
        <EmptyState message="No issues to display" />
      ) : (
        <div className="flex flex-col gap-1">
          {visible.map((issue) => (
            <IssueCard key={issue.id} issue={issue} onOpen={onOpen} />
          ))}
        </div>
      )}
    </section>
  );
}
