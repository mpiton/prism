import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types";
import { MyPRs } from "./index";

function makePr(
  overrides: Partial<PullRequestWithReview["pullRequest"]> = {},
): PullRequestWithReview {
  return {
    pullRequest: {
      id: `pr-${overrides.number ?? 1}`,
      number: 1,
      title: "Some PR",
      author: "me",
      state: "open",
      ciStatus: "success",
      priority: "medium",
      repoId: "repo-1",
      url: "https://github.com/org/repo/pull/1",
      labels: [],
      additions: 10,
      deletions: 2,
      createdAt: "2026-03-26T10:00:00Z",
      updatedAt: "2026-03-26T12:00:00Z",
      ...overrides,
    },
    reviewSummary: {
      totalReviews: 1,
      approved: 1,
      changesRequested: 0,
      pending: 0,
      reviewers: ["alice"],
    },
    workspace: null,
  };
}

const openPr1 = makePr({ number: 1, title: "Open PR one", state: "open" });
const openPr2 = makePr({ number: 2, title: "Open PR two", state: "open" });
const draftPr = makePr({ number: 3, title: "Draft PR", state: "draft" });
const mergedPr1 = makePr({ number: 4, title: "Merged PR one", state: "merged" });
const mergedPr2 = makePr({ number: 5, title: "Merged PR two", state: "merged" });

const allPrs = [openPr1, openPr2, draftPr, mergedPr1, mergedPr2];

const onOpen = vi.fn();

beforeEach(() => {
  onOpen.mockClear();
});

describe("MyPRs", () => {
  it("should show open PRs by default", () => {
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);

    expect(screen.getByText("Open PR one")).toBeInTheDocument();
    expect(screen.getByText("Open PR two")).toBeInTheDocument();
    expect(screen.getByText("Draft PR")).toBeInTheDocument();
    expect(screen.queryByText("Merged PR one")).not.toBeInTheDocument();
    expect(screen.queryByText("Merged PR two")).not.toBeInTheDocument();
  });

  it("should switch to merged tab", async () => {
    const user = userEvent.setup();
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: /merged/i }));

    expect(screen.getByText("Merged PR one")).toBeInTheDocument();
    expect(screen.getByText("Merged PR two")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();
    expect(screen.queryByText("Draft PR")).not.toBeInTheDocument();
  });

  it("should show correct counts", () => {
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);

    const openTab = screen.getByRole("button", { name: /open/i });
    const mergedTab = screen.getByRole("button", { name: /merged/i });

    expect(openTab).toHaveTextContent("3");
    expect(mergedTab).toHaveTextContent("2");
  });

  it("should show empty state when no PRs match current tab", () => {
    render(<MyPRs prs={[mergedPr1]} onOpen={onOpen} />);

    expect(screen.getByText(/no pull requests/i)).toBeInTheDocument();
  });

  it("should render SectionHead with title and total count", () => {
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);

    expect(screen.getByText("My PRs")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
  });

  it("should exclude closed PRs from total count", () => {
    const closedPr = makePr({ number: 6, title: "Closed PR", state: "closed" });
    render(<MyPRs prs={[openPr1, closedPr, mergedPr1]} onOpen={onOpen} />);

    expect(screen.getByText("My PRs")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.queryByText("Closed PR")).not.toBeInTheDocument();
  });

  it("should pass onOpen to MyPrCard", async () => {
    const user = userEvent.setup();
    render(<MyPRs prs={[openPr1]} onOpen={onOpen} />);

    await user.click(screen.getByText("Open PR one"));

    expect(onOpen).toHaveBeenCalledWith(openPr1.pullRequest.url);
  });
});
