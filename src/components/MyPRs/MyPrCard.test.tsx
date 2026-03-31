import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types";
import { MyPrCard } from "./MyPrCard";

const basePr: PullRequestWithReview = {
  pullRequest: {
    id: "pr-1",
    number: 99,
    title: "Add dashboard feature",
    author: "me",
    state: "open",
    ciStatus: "success",
    priority: "medium",
    repoId: "repo-1",
    url: "https://github.com/org/repo/pull/99",
    labels: [],
    additions: 42,
    deletions: 7,
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
  },
  reviewSummary: {
    totalReviews: 3,
    approved: 2,
    changesRequested: 0,
    pending: 1,
    reviewers: ["alice", "bob", "carol"],
  },
  workspace: {
    id: "ws-1",
    state: "active",
    lastNoteContent: null,
  },
};

describe("MyPrCard", () => {
  it("should render PR title and number", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    expect(screen.getByText("Add dashboard feature")).toBeInTheDocument();
    expect(screen.getByText("#99")).toBeInTheDocument();
  });

  it("should show CI dot", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const ciDot = screen.getByTestId("ci-dot");
    expect(ciDot).toBeInTheDocument();
  });

  it("should show diff stats", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    expect(screen.getByText("+42")).toBeInTheDocument();
    expect(screen.getByText("-7")).toBeInTheDocument();
  });

  it("should show CI badge", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    expect(screen.getByText("PASS")).toBeInTheDocument();
  });

  it("should show review dots", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const dots = screen.getAllByTestId("review-dot");
    expect(dots).toHaveLength(3);
  });

  it("should color review dots correctly (green=approved, red=changes, grey=pending)", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      reviewSummary: {
        totalReviews: 3,
        approved: 1,
        changesRequested: 1,
        pending: 1,
        reviewers: ["alice", "bob", "carol"],
      },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    const dots = screen.getAllByTestId("review-dot");
    expect(dots[0]).toHaveClass("bg-green");
    expect(dots[1]).toHaveClass("bg-red");
    expect(dots[2]).toHaveClass("bg-dim");
  });

  it("should show MERGEABLE when ci=pass && approved>0 && changes=0", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    expect(screen.getByText("MERGEABLE")).toBeInTheDocument();
  });

  it("should not show MERGEABLE when CI fails", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      pullRequest: { ...basePr.pullRequest, ciStatus: "failure" },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    expect(screen.queryByText("MERGEABLE")).not.toBeInTheDocument();
  });

  it("should not show MERGEABLE when changes requested", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      reviewSummary: {
        ...basePr.reviewSummary,
        changesRequested: 1,
      },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    expect(screen.queryByText("MERGEABLE")).not.toBeInTheDocument();
  });

  it("should not show MERGEABLE when no approvals", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      reviewSummary: {
        ...basePr.reviewSummary,
        approved: 0,
      },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    expect(screen.queryByText("MERGEABLE")).not.toBeInTheDocument();
  });

  it("should dim merged PRs", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      pullRequest: { ...basePr.pullRequest, state: "merged" },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    const row = screen.getByTestId("my-pr-card");
    expect(row).toHaveClass("opacity-50");
  });

  it("should show line-through on merged PR title", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      pullRequest: { ...basePr.pullRequest, state: "merged" },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    const title = screen.getByText("Add dashboard feature");
    expect(title).toHaveClass("line-through");
  });

  it("should not dim open PRs", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const row = screen.getByTestId("my-pr-card");
    expect(row).not.toHaveClass("opacity-50");
  });

  it("should show workspace badge", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    expect(screen.getByText("resume")).toBeInTheDocument();
  });

  it("should not show workspace badge when no workspace", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      workspace: null,
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    expect(screen.queryByText("resume")).not.toBeInTheDocument();
    expect(screen.queryByText("wake")).not.toBeInTheDocument();
    expect(screen.queryByText("open")).not.toBeInTheDocument();
  });

  it("should call onOpen on click", async () => {
    const handleOpen = vi.fn();
    render(<MyPrCard data={basePr} onOpen={handleOpen} />);
    await userEvent.click(screen.getByRole("link"));
    expect(handleOpen).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/99",
    );
  });

  it("should call onWorkspaceAction when badge clicked", async () => {
    const handleWs = vi.fn();
    render(
      <MyPrCard data={basePr} onOpen={vi.fn()} onWorkspaceAction={handleWs} />,
    );
    await userEvent.click(screen.getByRole("button"));
    expect(handleWs).toHaveBeenCalledWith("ws-1");
  });

  it("should not show MERGEABLE when PR is closed", () => {
    const data: PullRequestWithReview = {
      ...basePr,
      pullRequest: { ...basePr.pullRequest, state: "closed" },
    };
    render(<MyPrCard data={data} onOpen={vi.fn()} />);
    expect(screen.queryByText("MERGEABLE")).not.toBeInTheDocument();
  });

  it("should show time ago", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-26T14:00:00Z"));
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const timeElement = screen.getByTestId("time-ago");
    expect(timeElement).toHaveTextContent("2h");
    vi.useRealTimers();
  });

  it("should have aria-label on link describing the PR", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute("aria-label", "PR #99: Add dashboard feature");
  });

  it("should mark CI dot as aria-hidden", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const ciDot = screen.getByTestId("ci-dot");
    expect(ciDot).toHaveAttribute("aria-hidden", "true");
  });

  it("should mark review dots as aria-hidden", () => {
    render(<MyPrCard data={basePr} onOpen={vi.fn()} />);
    const dots = screen.getAllByTestId("review-dot");
    for (const dot of dots) {
      expect(dot).toHaveAttribute("aria-hidden", "true");
    }
  });
});
