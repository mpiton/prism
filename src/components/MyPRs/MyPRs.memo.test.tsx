import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import type { Repo } from "../../lib/types/github";

// Hoisted spies so the `vi.mock` factories (hoisted) can reference them.
const { myPrCardSpy, mockUseQuery } = vi.hoisted(() => ({
  myPrCardSpy: vi.fn(),
  mockUseQuery: vi.fn(),
}));

// Mock the `useQuery` used by MyPRs for listRepos so the component renders
// synchronously with a stable empty result.
vi.mock("@tanstack/react-query", async () => {
  const actual = await vi.importActual("@tanstack/react-query");
  return {
    ...actual,
    useQuery: mockUseQuery,
  };
});

// Replace MyPrCard with a light stub that counts render calls.
vi.mock("./MyPrCard", () => ({
  MyPrCard: (props: Record<string, unknown>) => {
    myPrCardSpy(props);
    return null;
  },
}));

// Import AFTER the mocks so the hoisted mocks win.
import { MyPRs } from "./MyPRs";

function makeRepo(): Repo {
  return {
    id: "repo-1",
    org: "org",
    name: "repo",
    fullName: "org/repo",
    url: "https://github.com/org/repo",
    defaultBranch: "main",
    isArchived: false,
    enabled: true,
    localPath: null,
    lastSyncAt: null,
  };
}

function makePr(number: number): PullRequestWithReview {
  return {
    pullRequest: {
      id: `pr-${number}`,
      number,
      title: `PR ${number}`,
      author: "me",
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

describe("MyPRs memoization", () => {
  beforeEach(() => {
    myPrCardSpy.mockClear();
    mockUseQuery.mockReturnValue({ data: [makeRepo()] });
  });

  it("should skip re-rendering child cards when props references are stable", () => {
    // Behavioural check for React.memo: mount once, then re-render with the
    // exact same prop references. A memoized component bails out of the
    // update, so MyPrCard is not invoked again.
    const prs: readonly PullRequestWithReview[] = [makePr(1), makePr(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<MyPRs prs={prs} onOpen={onOpen} />);
    const callsAfterMount = myPrCardSpy.mock.calls.length;
    expect(callsAfterMount).toBe(2);

    rerender(<MyPRs prs={prs} onOpen={onOpen} />);

    expect(myPrCardSpy.mock.calls.length).toBe(callsAfterMount);
  });

  it("should re-render child cards when props change", () => {
    // Sanity check: without stable refs, the component must still update.
    const initial: readonly PullRequestWithReview[] = [makePr(1)];
    const updated: readonly PullRequestWithReview[] = [makePr(1), makePr(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<MyPRs prs={initial} onOpen={onOpen} />);
    const callsAfterMount = myPrCardSpy.mock.calls.length;

    rerender(<MyPRs prs={updated} onOpen={onOpen} />);

    expect(myPrCardSpy.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });
});
