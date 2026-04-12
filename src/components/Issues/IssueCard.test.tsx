import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Issue } from "../../lib/types/github";
import { IssueCard } from "./IssueCard";

function makeIssue(overrides: Partial<Issue> = {}): Issue {
  return {
    id: "issue-1",
    number: 42,
    title: "Fix login bug",
    author: "alice",
    state: "open",
    priority: "medium",
    repoId: "repo-1",
    url: "https://github.com/org/repo/issues/42",
    labels: [],
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
    ...overrides,
  };
}

describe("IssueCard", () => {
  it("should render issue with title and number", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);

    expect(screen.getByText("Fix login bug")).toBeInTheDocument();
    expect(screen.getByText("#42")).toBeInTheDocument();
  });

  it("should show selected styling when keyboard-selected", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} isSelected />);
    const card = screen.getByTestId("issue-card");
    expect(card).toHaveAttribute("data-selected", "true");
    expect(card).toHaveAttribute("aria-current", "true");
    expect(card).toHaveClass("border-accent", "ring-2", "ring-accent");
  });

  it("should not set aria-current when not selected", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    expect(screen.getByTestId("issue-card")).not.toHaveAttribute("aria-current");
  });

  it("should render issue with labels", () => {
    render(
      <IssueCard
        issue={makeIssue({ labels: ["bug", "enhancement"] })}
        repoName="repo-name"
        onOpen={vi.fn()}
      />,
    );

    expect(screen.getByText("bug")).toBeInTheDocument();
    expect(screen.getByText("enhancement")).toBeInTheDocument();
  });

  it("should show green dot for open issues", () => {
    render(
      <IssueCard issue={makeIssue({ state: "open" })} repoName="repo-name" onOpen={vi.fn()} />,
    );

    const dot = screen.getByTestId("issue-state-dot");
    expect(dot.className).toContain("bg-green");
  });

  it("should show purple dot for closed issues", () => {
    render(
      <IssueCard issue={makeIssue({ state: "closed" })} repoName="repo-name" onOpen={vi.fn()} />,
    );

    const dot = screen.getByTestId("issue-state-dot");
    expect(dot.className).toContain("bg-purple");
  });

  it("should display repo name", () => {
    render(
      <IssueCard issue={makeIssue({ repoId: "repo-1" })} repoName="my-repo" onOpen={vi.fn()} />,
    );

    expect(screen.getByText("my-repo")).toBeInTheDocument();
  });

  it("should display relative time", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);

    expect(screen.getByTestId("time-ago")).toBeInTheDocument();
  });

  it("should call onOpen with url on click", async () => {
    const onOpen = vi.fn();
    const user = userEvent.setup();

    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={onOpen} />);

    await user.click(screen.getByText("Fix login bug"));

    expect(onOpen).toHaveBeenCalledWith("https://github.com/org/repo/issues/42");
  });

  it("should have aria-label on link describing the issue", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("aria-label", "Issue #42: Fix login bug (open)");
  });

  it("should mark state dot as aria-hidden", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    const dot = screen.getByTestId("issue-state-dot");
    expect(dot).toHaveAttribute("aria-hidden", "true");
  });

  it("should have title attribute on issue title span", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    const titleSpan = screen.getByText("Fix login bug");
    expect(titleSpan).toHaveAttribute("title", "Fix login bug");
  });

  it("should have title attribute on repo name span", () => {
    render(<IssueCard issue={makeIssue()} repoName="my-repo" onOpen={vi.fn()} />);
    const repoSpan = screen.getByText("my-repo");
    expect(repoSpan).toHaveAttribute("title", "my-repo");
  });

  it("should render external link indicator", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    const indicator = screen.getByTestId("external-link-indicator");
    expect(indicator).toHaveTextContent("↗");
  });

  it("should mark external link indicator as aria-hidden", () => {
    render(<IssueCard issue={makeIssue()} repoName="repo-name" onOpen={vi.fn()} />);
    const indicator = screen.getByTestId("external-link-indicator");
    expect(indicator).toHaveAttribute("aria-hidden", "true");
  });
});
