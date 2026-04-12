import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_RING } from "../../lib/a11y";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import type { Repo } from "../../lib/types/github";
import { MyPRs } from "./MyPRs";

const { mockUseQuery } = vi.hoisted(() => ({ mockUseQuery: vi.fn() }));

vi.mock("@tanstack/react-query", async () => {
  const actual = await vi.importActual("@tanstack/react-query");
  return {
    ...actual,
    useQuery: mockUseQuery,
  };
});

function makeRepo(overrides: Partial<Repo> = {}): Repo {
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
    ...overrides,
  };
}

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
      headRefName: "fix/test",
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
  mockUseQuery.mockReturnValue({ data: [makeRepo()] });
});

describe("MyPRs", () => {
  it("should apply the focus-visible ring to the search input (WCAG 2.4.7)", () => {
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);
    const search = screen.getByRole("searchbox", { name: /filter prs/i });
    for (const token of FOCUS_RING.trim().split(/\s+/)) {
      expect(search).toHaveClass(token);
    }
  });

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

    const group = screen.getByRole("group", { name: /filter by state/i });
    const buttons = within(group).getAllByRole("button");
    const openTab = buttons[0];
    const mergedTab = buttons[1];

    expect(openTab).toHaveTextContent("3"); // openPr1, openPr2, draftPr
    expect(mergedTab).toHaveTextContent("2"); // mergedPr1, mergedPr2
  });

  it("should filter PRs by title, author, repo, and labels", async () => {
    const user = userEvent.setup();
    const labeledPr = makePr({
      number: 6,
      title: "Refine search interaction",
      author: "alice",
      repoId: "repo-2",
      labels: ["ux"],
      state: "open",
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<MyPRs prs={[openPr1, labeledPr, mergedPr1]} onOpen={onOpen} />);

    const input = screen.getByPlaceholderText("Filter PRs...");

    await user.type(input, "refine");
    expect(screen.getByText("Refine search interaction")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "alice");
    expect(screen.getByText("Refine search interaction")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "console");
    expect(screen.getByText("Refine search interaction")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "ux");
    expect(screen.getByText("Refine search interaction")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "merged");
    await user.click(screen.getByRole("button", { name: /merged/i }));
    expect(screen.getByText("Merged PR one")).toBeInTheDocument();
    expect(screen.queryByText("Refine search interaction")).not.toBeInTheDocument();
  });

  it("should keep state filters at the minimum touch target size", () => {
    render(<MyPRs prs={allPrs} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by state/i });
    const buttons = within(group).getAllByRole("button");

    for (const button of buttons) {
      expect(button).toHaveClass("min-h-11", "min-w-11");
    }
  });

  it("should show empty state when no PRs match current tab", () => {
    render(<MyPRs prs={[mergedPr1]} onOpen={onOpen} />);

    expect(screen.getByText(/no pull requests/i)).toBeInTheDocument();
  });

  it("should render card skeletons while loading", () => {
    render(<MyPRs prs={[]} isLoading onOpen={onOpen} />);

    expect(screen.getByTestId("my-prs")).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByTestId("my-pr-card-skeleton")).toHaveLength(3);
    expect(screen.queryByText(/no pull requests/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /filter by state/i })).not.toBeInTheDocument();
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

  it("should show repo dropdown when multiple repos exist", () => {
    const pr2 = makePr({ number: 6, title: "PR from acme", repoId: "repo-2", state: "open" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<MyPRs prs={[openPr1, pr2]} onOpen={onOpen} />);

    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    expect(select).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "All repos" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "org/repo" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "acme/console" })).toBeInTheDocument();
  });

  it("should hide repo dropdown when single repo", () => {
    render(<MyPRs prs={[openPr1, openPr2]} onOpen={onOpen} />);

    expect(screen.queryByRole("combobox", { name: /filter by repo/i })).not.toBeInTheDocument();
  });

  it("should filter PRs by selected repo", async () => {
    const pr2 = makePr({ number: 6, title: "PR from acme", repoId: "repo-2", state: "open" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<MyPRs prs={[openPr1, pr2]} onOpen={onOpen} />);

    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    await userEvent.selectOptions(select, "repo-2");

    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();
    expect(screen.getByText("PR from acme")).toBeInTheDocument();
  });

  it("should show label filter buttons when labels exist", () => {
    const bugPr = makePr({ number: 6, title: "Bug fix", labels: ["bug"], state: "open" });
    const featurePr = makePr({ number: 7, title: "New feature", labels: ["feature"], state: "open" });

    render(<MyPRs prs={[bugPr, featurePr]} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by label/i });
    expect(within(group).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "feature" })).toBeInTheDocument();
  });

  it("should hide label filters when no labels", () => {
    render(<MyPRs prs={[openPr1, openPr2]} onOpen={onOpen} />);

    expect(screen.queryByRole("group", { name: /filter by label/i })).not.toBeInTheDocument();
  });

  it("should filter PRs by selected label", async () => {
    const user = userEvent.setup();
    const bugPr = makePr({ number: 6, title: "Bug fix", labels: ["bug"], state: "open" });
    const featurePr = makePr({ number: 7, title: "New feature", labels: ["feature"], state: "open" });

    render(<MyPRs prs={[bugPr, featurePr]} onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: "bug" }));

    expect(screen.getByText("Bug fix")).toBeInTheDocument();
    expect(screen.queryByText("New feature")).not.toBeInTheDocument();
  });

  it("should deselect label filter on second click", async () => {
    const user = userEvent.setup();
    const bugPr = makePr({ number: 6, title: "Bug fix", labels: ["bug"], state: "open" });
    const featurePr = makePr({ number: 7, title: "New feature", labels: ["feature"], state: "open" });

    render(<MyPRs prs={[bugPr, featurePr]} onOpen={onOpen} />);

    const bugButton = screen.getByRole("button", { name: "bug" });
    await user.click(bugButton);
    await user.click(bugButton);

    expect(screen.getByText("Bug fix")).toBeInTheDocument();
    expect(screen.getByText("New feature")).toBeInTheDocument();
  });

  it("should combine repo + label + search + tab filters", async () => {
    const user = userEvent.setup();
    const pr2 = makePr({ number: 6, title: "Acme bug fix", repoId: "repo-2", labels: ["bug"], state: "open" });
    const pr3 = makePr({ number: 7, title: "Acme feature", repoId: "repo-2", labels: ["feature"], state: "open" });
    const pr4 = makePr({ number: 8, title: "Acme merged bug", repoId: "repo-2", labels: ["bug"], state: "merged" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<MyPRs prs={[openPr1, pr2, pr3, pr4]} onOpen={onOpen} />);

    // Select repo-2
    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    await userEvent.selectOptions(select, "repo-2");

    // Select "bug" label
    await user.click(screen.getByRole("button", { name: "bug" }));

    // Search for "acme"
    await user.type(screen.getByPlaceholderText("Filter PRs..."), "acme");

    // Tab is still "open" by default
    expect(screen.getByText("Acme bug fix")).toBeInTheDocument();
    expect(screen.queryByText("Open PR one")).not.toBeInTheDocument();
    expect(screen.queryByText("Acme feature")).not.toBeInTheDocument();
    expect(screen.queryByText("Acme merged bug")).not.toBeInTheDocument();
  });

  it("should reset stale repo/label filters after data changes", async () => {
    const pr1 = makePr({ number: 1, title: "PR repo1", repoId: "repo-1", labels: ["bug"], state: "open" });
    const pr2 = makePr({ number: 2, title: "PR repo2", repoId: "repo-2", labels: ["feature"], state: "open" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    const { rerender } = render(<MyPRs prs={[pr1, pr2]} onOpen={onOpen} />);

    // Select repo-2 and label "feature"
    await userEvent.selectOptions(screen.getByRole("combobox", { name: /filter by repo/i }), "repo-2");
    await userEvent.click(screen.getByRole("button", { name: "feature" }));

    expect(screen.getByText("PR repo2")).toBeInTheDocument();
    expect(screen.queryByText("PR repo1")).not.toBeInTheDocument();

    // Rerender with data that no longer includes repo-2 or "feature" label
    const pr3 = makePr({ number: 3, title: "PR repo1 new", repoId: "repo-1", labels: ["docs"], state: "open" });
    mockUseQuery.mockReturnValue({ data: [makeRepo()] });
    rerender(<MyPRs prs={[pr1, pr3]} onOpen={onOpen} />);

    // Filters should reset — all items visible again
    expect(screen.getByText("PR repo1")).toBeInTheDocument();
    expect(screen.getByText("PR repo1 new")).toBeInTheDocument();
    // Repo dropdown hidden (single repo), label group shows new labels
    expect(screen.queryByRole("combobox", { name: /filter by repo/i })).not.toBeInTheDocument();
  });

  it("should show only labels from the selected repo", async () => {
    const pr1 = makePr({ number: 1, title: "PR1", repoId: "repo-1", labels: ["bug"], state: "open" });
    const pr2 = makePr({ number: 2, title: "PR2", repoId: "repo-2", labels: ["feature"], state: "open" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<MyPRs prs={[pr1, pr2]} onOpen={onOpen} />);

    // Before repo filter: both labels visible
    const labelGroup = screen.getByRole("group", { name: /filter by label/i });
    expect(within(labelGroup).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(labelGroup).getByRole("button", { name: "feature" })).toBeInTheDocument();

    // Select repo-1: only "bug" label should remain
    await userEvent.selectOptions(screen.getByRole("combobox", { name: /filter by repo/i }), "repo-1");
    expect(within(labelGroup).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(labelGroup).queryByRole("button", { name: "feature" })).not.toBeInTheDocument();
  });

});
