import { useQuery } from "@tanstack/react-query";
import { type ReactElement, useMemo, useState } from "react";
import { listRepos } from "../../lib/tauri";
import type { Issue } from "../../lib/types";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
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

  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();
    const nameCounts = new Map<string, number>();
    for (const repo of repos) {
      nameCounts.set(repo.name, (nameCounts.get(repo.name) ?? 0) + 1);
    }
    return new Map(
      repos.map((repo) => [
        repo.id,
        nameCounts.get(repo.name) === 1 ? repo.name : repo.fullName,
      ]),
    );
  }, [repos]);

  const openIssues = issues.filter(isOpen);
  const closedIssues = issues.filter(isClosed);
  const visible = tab === "open" ? openIssues : closedIssues;

  const navItems = useMemo(
    () =>
      issues
        .filter(tab === "open" ? isOpen : isClosed)
        .map((issue) => ({ url: issue.url })),
    [issues, tab],
  );
  useRegisterNavigableItems(navItems);

  return (
    <section data-testid="issues" className="flex flex-col gap-2">
      <SectionHead title="Issues" count={issues.length} />

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
          Open {openIssues.length}
        </button>
        <button
          type="button"
          aria-pressed={tab === "closed"}
          onClick={() => setTab("closed")}
          className={`rounded px-2 py-0.5 text-xs transition-colors ${
            tab === "closed"
              ? "bg-accent text-bg font-semibold"
              : "text-dim hover:text-foreground"
          }`}
        >
          Closed {closedIssues.length}
        </button>
      </div>

      {visible.length === 0 ? (
        <EmptyState icon="◎" message="No issues to display" />
      ) : (
        <div className="flex flex-col gap-1">
          {visible.map((issue) => (
            <IssueCard key={issue.id} issue={issue} repoName={repoMap.get(issue.repoId) ?? issue.repoId} onOpen={onOpen} />
          ))}
        </div>
      )}
    </section>
  );
}
