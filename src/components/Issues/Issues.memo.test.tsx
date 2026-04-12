import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Issue, Repo } from "../../lib/types/github";

// Hoisted spies so the `vi.mock` factories (hoisted) can reference them.
const { issueCardSpy, mockUseQuery } = vi.hoisted(() => ({
  issueCardSpy: vi.fn(),
  mockUseQuery: vi.fn(),
}));

// Mock useVirtualizer so every item is always in the virtual window (deterministic).
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (opts: { count: number; estimateSize: (i: number) => number }) => ({
    getVirtualItems: () =>
      Array.from({ length: opts.count }, (_, i) => ({
        index: i,
        key: i,
        start: i * opts.estimateSize(i),
        size: opts.estimateSize(i),
      })),
    getTotalSize: () => opts.count * opts.estimateSize(0),
  }),
}));

// Mock useQuery for the `listRepos` call inside Issues.
vi.mock("@tanstack/react-query", async () => {
  const actual = await vi.importActual("@tanstack/react-query");
  return {
    ...actual,
    useQuery: mockUseQuery,
  };
});

// Memoization tests should isolate child renders from dashboard registration side effects.
vi.mock("../../hooks/useRegisterNavigableItems", () => ({
  useRegisterNavigableItems: vi.fn(),
}));

// Replace IssueCard with a light stub that counts render calls.
vi.mock("./IssueCard", () => ({
  IssueCard: (props: Record<string, unknown>) => {
    issueCardSpy(props);
    return null;
  },
}));

// Import AFTER the mocks so the hoisted mocks win.
import { Issues } from "./Issues";

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

function makeIssue(number: number): Issue {
  return {
    id: `issue-${number}`,
    number,
    title: `Issue ${number}`,
    author: "alice",
    state: "open",
    priority: "medium",
    repoId: "repo-1",
    url: `https://github.com/org/repo/issues/${number}`,
    labels: [],
    createdAt: "2026-04-11T10:00:00Z",
    updatedAt: "2026-04-11T10:00:00Z",
  };
}

describe("Issues memoization", () => {
  beforeEach(() => {
    issueCardSpy.mockClear();
    mockUseQuery.mockReturnValue({ data: [makeRepo()] });
  });

  it("should skip re-rendering child cards when props references are stable", () => {
    // Behavioural check for React.memo: mount once, then re-render with the
    // exact same prop references. A memoized component bails out of the
    // update, so IssueCard is not invoked again.
    const issues: readonly Issue[] = [makeIssue(1), makeIssue(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<Issues issues={issues} onOpen={onOpen} />);
    const callsAfterMount = issueCardSpy.mock.calls.length;
    expect(callsAfterMount).toBe(2);

    rerender(<Issues issues={issues} onOpen={onOpen} />);

    expect(issueCardSpy.mock.calls.length).toBe(callsAfterMount);
  });

  it("should re-render child cards when props change", () => {
    // Sanity check: without stable refs, the component must still update.
    const initial: readonly Issue[] = [makeIssue(1)];
    const updated: readonly Issue[] = [makeIssue(1), makeIssue(2)];
    const onOpen = vi.fn();

    const { rerender } = render(<Issues issues={initial} onOpen={onOpen} />);
    const callsAfterMount = issueCardSpy.mock.calls.length;

    rerender(<Issues issues={updated} onOpen={onOpen} />);

    expect(issueCardSpy.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });
});
