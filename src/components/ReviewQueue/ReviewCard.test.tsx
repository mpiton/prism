import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import { ReviewCard } from "./ReviewCard";

const mockData: PullRequestWithReview = {
  pullRequest: {
    id: "pr-1",
    number: 42,
    title: "Fix login bug",
    author: "alice",
    state: "open",
    ciStatus: "success",
    priority: "high",
    repoId: "repo-1",
    url: "https://github.com/org/repo/pull/42",
    headRefName: "fix/test",
    labels: ["bug"],
    additions: 10,
    deletions: 5,
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
  },
  reviewSummary: {
    totalReviews: 3,
    approved: 1,
    changesRequested: 0,
    pending: 2,
    reviewers: ["bob", "carol", "dave"],
  },
  workspace: {
    id: "ws-1",
    state: "active",
    lastNoteContent: null,
  },
};

describe("ReviewCard", () => {
  it("should render PR title and number", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("Fix login bug")).toBeInTheDocument();
    expect(screen.getByText("#42")).toBeInTheDocument();
  });

  it("should render author name", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("alice")).toBeInTheDocument();
  });

  it("should show diff stats", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("+10")).toBeInTheDocument();
    expect(screen.getByText("-5")).toBeInTheDocument();
  });

  it("should show CI status", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    expect(screen.getByText("PASS")).toBeInTheDocument();
  });

  it("should show workspace badge", () => {
    render(
      <ReviewCard
        data={mockData}
        onOpen={vi.fn()}
        onWorkspaceAction={vi.fn()}
      />,
    );
    expect(screen.getByText("resume")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Resume workspace for PR #42" }),
    ).toBeInTheDocument();
  });

  it("should call onOpen on click", async () => {
    const handleOpen = vi.fn();
    render(<ReviewCard data={mockData} onOpen={handleOpen} />);
    await userEvent.click(screen.getByRole("link"));
    expect(handleOpen).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/42",
    );
  });

  it("should render without optional diff fields", () => {
    const dataWithoutDiff: PullRequestWithReview = {
      pullRequest: {
        ...mockData.pullRequest,
        additions: undefined,
        deletions: undefined,
      },
      reviewSummary: mockData.reviewSummary,
      workspace: null,
    };
    render(<ReviewCard data={dataWithoutDiff} onOpen={vi.fn()} />);
    expect(screen.queryByText("+10")).not.toBeInTheDocument();
    expect(screen.getByText("Fix login bug")).toBeInTheDocument();
  });

  it("should call onWorkspaceAction when badge is clicked", async () => {
    const handleWs = vi.fn();
    render(
      <ReviewCard
        data={mockData}
        onOpen={vi.fn()}
        onWorkspaceAction={handleWs}
      />,
    );
    await userEvent.click(screen.getByRole("button"));
    expect(handleWs).toHaveBeenCalledWith({
      repoId: "repo-1",
      pullRequestNumber: 42,
      headRefName: "fix/test",
      workspaceId: "ws-1",
      workspaceState: "active",
    });
  });

  it("should have aria-label on link describing the PR", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("aria-label", "PR #42: Fix login bug by alice");
  });

  it("should NOT render WsBadge when PR is closed with suspended workspace", () => {
    const data: PullRequestWithReview = {
      ...mockData,
      pullRequest: { ...mockData.pullRequest, state: "closed" },
      workspace: { id: "ws-1", state: "suspended", lastNoteContent: null },
    };
    render(
      <ReviewCard data={data} onOpen={vi.fn()} onWorkspaceAction={vi.fn()} />,
    );
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("should have title attribute on PR title span", () => {
    render(<ReviewCard data={mockData} onOpen={vi.fn()} />);
    const titleSpan = screen.getByText("Fix login bug");
    expect(titleSpan).toHaveAttribute("title", "Fix login bug");
  });
});
