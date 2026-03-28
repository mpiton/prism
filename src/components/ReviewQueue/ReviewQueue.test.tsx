import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types";
import { ReviewQueue } from "./ReviewQueue";

function makePr(
  overrides: Partial<PullRequestWithReview["pullRequest"]> = {},
): PullRequestWithReview {
  return {
    pullRequest: {
      id: overrides.id ?? `pr-${overrides.number ?? 1}`,
      number: 1,
      title: "Default PR",
      author: "alice",
      state: "open",
      ciStatus: "success",
      priority: "medium",
      repoId: "repo-1",
      url: "https://github.com/org/repo/pull/1",
      labels: [],
      additions: 10,
      deletions: 5,
      createdAt: "2026-03-26T10:00:00Z",
      updatedAt: "2026-03-26T12:00:00Z",
      ...overrides,
    },
    reviewSummary: {
      totalReviews: 1,
      approved: 0,
      changesRequested: 0,
      pending: 1,
      reviewers: ["bob"],
    },
    workspace: null,
  };
}

const criticalPr = makePr({
  number: 10,
  title: "Critical fix",
  priority: "critical",
  url: "https://github.com/org/repo/pull/10",
});

const highPr = makePr({
  number: 20,
  title: "High feature",
  priority: "high",
  url: "https://github.com/org/repo/pull/20",
});

const mediumPr = makePr({
  number: 30,
  title: "Medium task",
  priority: "medium",
  url: "https://github.com/org/repo/pull/30",
});

const lowPr = makePr({
  number: 40,
  title: "Low chore",
  priority: "low",
  url: "https://github.com/org/repo/pull/40",
});

const allReviews = [lowPr, criticalPr, mediumPr, highPr];

describe("ReviewQueue", () => {
  it("should render PRs sorted by priority", () => {
    render(<ReviewQueue reviews={allReviews} onOpen={vi.fn()} />);

    const cards = screen.getAllByRole("link");
    const titles = cards.map((c) => within(c).getByText(/fix|feature|task|chore/i).textContent);

    expect(titles).toEqual([
      "Critical fix",
      "High feature",
      "Medium task",
      "Low chore",
    ]);
  });

  it("should filter by priority", async () => {
    const user = userEvent.setup();
    render(<ReviewQueue reviews={allReviews} onOpen={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: /critical/i }));

    const cards = screen.getAllByRole("link");
    expect(cards).toHaveLength(1);
    expect(screen.getByText("Critical fix")).toBeInTheDocument();
    expect(screen.queryByText("High feature")).not.toBeInTheDocument();
  });

  it("should show empty state when no reviews", () => {
    render(<ReviewQueue reviews={[]} onOpen={vi.fn()} />);

    expect(screen.getByText(/no.*review/i)).toBeInTheDocument();
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });

  it("should show section header with count", () => {
    render(<ReviewQueue reviews={allReviews} onOpen={vi.fn()} />);

    expect(screen.getByText("Reviews")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  it("should show all PRs when 'all' filter is selected", async () => {
    const user = userEvent.setup();
    render(<ReviewQueue reviews={allReviews} onOpen={vi.fn()} />);

    // First filter to critical
    await user.click(screen.getByRole("button", { name: /critical/i }));
    expect(screen.getAllByRole("link")).toHaveLength(1);

    // Then back to all
    await user.click(screen.getByRole("button", { name: /all/i }));
    expect(screen.getAllByRole("link")).toHaveLength(4);
  });

  it("should update section header count when filtering", async () => {
    const user = userEvent.setup();
    render(<ReviewQueue reviews={allReviews} onOpen={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: /high/i }));

    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("should show empty state when filter matches nothing", async () => {
    const user = userEvent.setup();
    const reviews = [highPr];
    render(<ReviewQueue reviews={reviews} onOpen={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: /critical/i }));

    expect(screen.getByText(/no.*review/i)).toBeInTheDocument();
  });

  it("should pass onOpen and onWorkspaceAction to ReviewCard", async () => {
    const handleOpen = vi.fn();
    const handleWs = vi.fn();
    render(
      <ReviewQueue
        reviews={[highPr]}
        onOpen={handleOpen}
        onWorkspaceAction={handleWs}
      />,
    );

    await userEvent.click(screen.getByRole("link"));
    expect(handleOpen).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/20",
    );
  });
});
