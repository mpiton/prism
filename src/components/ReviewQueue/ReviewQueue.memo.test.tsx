import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import { useDashboardStore } from "../../stores/dashboard";

// Hoisted spy so the `vi.mock` factory (which is hoisted) can reference it.
const { reviewCardSpy } = vi.hoisted(() => ({ reviewCardSpy: vi.fn() }));

// Memoization tests should isolate child renders from dashboard registration side effects.
vi.mock("../../hooks/useRegisterNavigableItems", () => ({
  useRegisterNavigableItems: vi.fn(),
}));

// Replace ReviewCard with a light stub that counts render calls.
vi.mock("./ReviewCard", () => ({
  ReviewCard: (props: Record<string, unknown>) => {
    reviewCardSpy(props);
    return null;
  },
}));

// Import AFTER the mock so the hoisted mock wins.
import { ReviewQueue } from "./ReviewQueue";

function makePr(number: number): PullRequestWithReview {
  return {
    pullRequest: {
      id: `pr-${number}`,
      number,
      title: `PR ${number}`,
      author: "test",
      state: "open",
      ciStatus: "success",
      priority: "medium",
      repoId: "repo-1",
      url: `https://github.com/org/repo/pull/${number}`,
      headRefName: `fix/test-${number}`,
      labels: [],
      additions: 0,
      deletions: 0,
      createdAt: "2026-04-11T10:00:00Z",
      updatedAt: "2026-04-11T10:00:00Z",
    },
    reviewSummary: {
      totalReviews: 0,
      approved: 0,
      changesRequested: 0,
      pending: 0,
      reviewers: [],
    },
    workspace: null,
  };
}

describe("ReviewQueue memoization", () => {
  beforeEach(() => {
    reviewCardSpy.mockClear();
    useDashboardStore.setState({ activeFilters: {}, focusMode: false });
  });

  it("should skip re-rendering child cards when props references are stable", () => {
    // Behavioural check for React.memo: mount once, then re-render with the
    // exact same prop references. A memoized component bails out of the
    // update, so its children (here: ReviewCard) are not invoked again.
    const reviews: readonly PullRequestWithReview[] = [makePr(1), makePr(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<ReviewQueue reviews={reviews} onOpen={onOpen} />);
    const callsAfterMount = reviewCardSpy.mock.calls.length;
    expect(callsAfterMount).toBe(2);

    rerender(<ReviewQueue reviews={reviews} onOpen={onOpen} />);

    expect(reviewCardSpy.mock.calls.length).toBe(callsAfterMount);
  });

  it("should re-render child cards when props change", () => {
    // Sanity check: without stable refs, the component must still update.
    const initial: readonly PullRequestWithReview[] = [makePr(1)];
    const updated: readonly PullRequestWithReview[] = [makePr(1), makePr(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<ReviewQueue reviews={initial} onOpen={onOpen} />);
    const callsAfterMount = reviewCardSpy.mock.calls.length;

    rerender(<ReviewQueue reviews={updated} onOpen={onOpen} />);

    expect(reviewCardSpy.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });
});
