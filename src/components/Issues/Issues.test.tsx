import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_RING } from "../../lib/a11y";
import type { Issue, Repo } from "../../lib/types/github";
import { useDashboardStore } from "../../stores/dashboard";
import { Issues, LABEL_VISIBLE_LIMIT } from "./Issues";

const { mockScrollToIndex } = vi.hoisted(() => ({ mockScrollToIndex: vi.fn() }));

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
    scrollToIndex: mockScrollToIndex,
  }),
}));

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

function makeIssue(overrides: Partial<Issue> = {}): Issue {
  return {
    id: `issue-${overrides.number ?? 1}`,
    number: 1,
    title: "Some issue",
    author: "alice",
    state: "open",
    priority: "medium",
    repoId: "repo-1",
    url: "https://github.com/org/repo/issues/1",
    labels: [],
    createdAt: "2026-03-26T10:00:00Z",
    updatedAt: "2026-03-26T12:00:00Z",
    ...overrides,
  };
}

function makeLabels(count: number): string[] {
  return Array.from({ length: count }, (_, i) => `label-${String(i + 1).padStart(2, "0")}`);
}

const openIssue1 = makeIssue({ number: 1, title: "Open issue one", state: "open" });
const openIssue2 = makeIssue({ number: 2, title: "Open issue two", state: "open" });
const closedIssue1 = makeIssue({ number: 3, title: "Closed issue one", state: "closed" });
const closedIssue2 = makeIssue({ number: 4, title: "Closed issue two", state: "closed" });

const allIssues = [openIssue1, openIssue2, closedIssue1, closedIssue2];

const onOpen = vi.fn();

beforeEach(() => {
  onOpen.mockClear();
  mockUseQuery.mockReturnValue({ data: [makeRepo()] });
  mockScrollToIndex.mockReset();
  useDashboardStore.setState({
    activeNavigableSection: null,
    navigableSectionRegistrations: [],
    selectedIndex: -1,
    navigableItems: [],
  });
});

describe("Issues", () => {
  it("should apply the focus-visible ring to the search input (WCAG 2.4.7)", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);
    const search = screen.getByRole("searchbox", { name: /filter issues/i });
    for (const token of FOCUS_RING.trim().split(/\s+/)) {
      expect(search).toHaveClass(token);
    }
  });

  it("should show open issues by default", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    expect(screen.getByText("Open issue one")).toBeInTheDocument();
    expect(screen.getByText("Open issue two")).toBeInTheDocument();
    expect(screen.queryByText("Closed issue one")).not.toBeInTheDocument();
    expect(screen.queryByText("Closed issue two")).not.toBeInTheDocument();
  });

  it("should filter open/closed", async () => {
    const user = userEvent.setup();
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: /closed/i }));

    expect(screen.getByText("Closed issue one")).toBeInTheDocument();
    expect(screen.getByText("Closed issue two")).toBeInTheDocument();
    expect(screen.queryByText("Open issue one")).not.toBeInTheDocument();
    expect(screen.queryByText("Open issue two")).not.toBeInTheDocument();
  });

  it("should scroll the virtualizer to the selected issue during keyboard navigation", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    act(() => {
      useDashboardStore.setState({
        activeNavigableSection: "issues",
        selectedIndex: 1,
      });
    });

    return waitFor(() => {
      expect(mockScrollToIndex).toHaveBeenCalledWith(1, { align: "auto" });
    });
  });

  it("should show correct counts in tabs", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    const openTab = screen.getByRole("button", { name: /open/i });
    const closedTab = screen.getByRole("button", { name: /closed/i });

    expect(openTab).toHaveTextContent("2");
    expect(closedTab).toHaveTextContent("2");
  });

  it("should filter issues by title, author, repo name, and labels", async () => {
    const user = userEvent.setup();
    const labeledIssue = makeIssue({
      number: 5,
      title: "Refine search flow",
      author: "bob",
      repoId: "repo-2",
      labels: ["ux"],
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[openIssue1, labeledIssue]} onOpen={onOpen} />);

    const input = screen.getByPlaceholderText("Filter issues...");

    await user.type(input, "search");
    expect(screen.getByText("Refine search flow")).toBeInTheDocument();
    expect(screen.queryByText("Open issue one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "bob");
    expect(screen.getByText("Refine search flow")).toBeInTheDocument();
    expect(screen.queryByText("Open issue one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "console");
    expect(screen.getByText("Refine search flow")).toBeInTheDocument();
    expect(screen.queryByText("Open issue one")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "ux");
    expect(screen.getByText("Refine search flow")).toBeInTheDocument();
    expect(screen.queryByText("Open issue one")).not.toBeInTheDocument();
  });

  it("should keep state filters at the minimum touch target size", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by state/i });
    const buttons = within(group).getAllByRole("button");

    for (const button of buttons) {
      expect(button).toHaveClass("min-h-11", "min-w-11");
    }
  });

  it("should show empty state when no open issues", () => {
    render(<Issues issues={[closedIssue1]} onOpen={onOpen} />);

    expect(screen.getByText(/no issues/i)).toBeInTheDocument();
  });

  it("should render list skeletons while loading", () => {
    render(<Issues issues={[]} isLoading onOpen={onOpen} />);

    expect(screen.getByTestId("issues")).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByTestId("issue-card-skeleton")).toHaveLength(4);
    expect(screen.queryByText(/no issues/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /filter by state/i })).not.toBeInTheDocument();
  });

  it("should show empty state on closed tab when no closed issues", async () => {
    const user = userEvent.setup();
    render(<Issues issues={[openIssue1]} onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: /closed/i }));

    expect(screen.getByText(/no issues/i)).toBeInTheDocument();
  });

  it("should render SectionHead with title and total count", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} />);

    expect(screen.getByText("Issues")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  it("should hide section header when hideHeader is true", () => {
    render(<Issues issues={allIssues} onOpen={onOpen} hideHeader />);

    expect(screen.queryByText("Issues")).not.toBeInTheDocument();
  });

  it("should pass onOpen to IssueCard", async () => {
    const user = userEvent.setup();
    render(<Issues issues={[openIssue1]} onOpen={onOpen} />);

    await user.click(screen.getByText("Open issue one"));

    expect(onOpen).toHaveBeenCalledWith(openIssue1.url);
  });

  it("should display fullName instead of repo id", () => {
    render(<Issues issues={[openIssue1]} onOpen={onOpen} />);

    expect(screen.getByText("org/repo")).toBeInTheDocument();
  });

  it("should always display fullName for all repos", () => {
    const issue1 = makeIssue({
      number: 1,
      title: "Issue in org-a",
      state: "open",
      repoId: "repo-a",
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Issue in org-b",
      state: "open",
      repoId: "repo-b",
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo({ id: "repo-a", org: "org-a", name: "shared", fullName: "org-a/shared" }),
        makeRepo({ id: "repo-b", org: "org-b", name: "shared", fullName: "org-b/shared" }),
      ],
    });

    render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    expect(screen.getAllByText("org-a/shared").length).toBeGreaterThan(0);
    expect(screen.getAllByText("org-b/shared").length).toBeGreaterThan(0);
  });

  it("should fallback to repoId when repo not found in map", () => {
    const orphanIssue = makeIssue({
      number: 99,
      title: "Orphan issue",
      state: "open",
      repoId: "unknown-repo",
    });

    render(<Issues issues={[orphanIssue]} onOpen={onOpen} />);

    expect(screen.getByText("unknown-repo")).toBeInTheDocument();
  });

  it("should show repo dropdown when multiple repos exist", () => {
    const issue1 = makeIssue({
      number: 1,
      title: "Issue in repo 1",
      state: "open",
      repoId: "repo-1",
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Issue in repo 2",
      state: "open",
      repoId: "repo-2",
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    expect(select).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "All repos" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "acme/console" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "org/repo" })).toBeInTheDocument();
  });

  it("should hide repo dropdown when single repo", () => {
    render(<Issues issues={[openIssue1, openIssue2]} onOpen={onOpen} />);

    expect(screen.queryByRole("combobox", { name: /filter by repo/i })).toBeNull();
  });

  it("should filter issues by selected repo", async () => {
    const issue1 = makeIssue({ number: 1, title: "Repo 1 issue", state: "open", repoId: "repo-1" });
    const issue2 = makeIssue({ number: 2, title: "Repo 2 issue", state: "open", repoId: "repo-2" });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    await userEvent.selectOptions(select, "repo-2");

    expect(screen.getByText("Repo 2 issue")).toBeInTheDocument();
    expect(screen.queryByText("Repo 1 issue")).not.toBeInTheDocument();
  });

  it("should show label filter buttons when labels exist", () => {
    const labeledIssue = makeIssue({
      number: 5,
      title: "Bug issue",
      state: "open",
      labels: ["bug", "help wanted"],
    });

    render(<Issues issues={[labeledIssue]} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by label/i });
    expect(group).toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(group).getByRole("button", { name: "help wanted" })).toBeInTheDocument();
  });

  it("should hide label filters when no labels", () => {
    render(<Issues issues={[openIssue1]} onOpen={onOpen} />);

    expect(screen.queryByRole("group", { name: /filter by label/i })).toBeNull();
  });

  it("should filter issues by selected label", async () => {
    const user = userEvent.setup();
    const bugIssue = makeIssue({ number: 1, title: "Bug issue", state: "open", labels: ["bug"] });
    const featureIssue = makeIssue({
      number: 2,
      title: "Feature issue",
      state: "open",
      labels: ["feature"],
    });

    render(<Issues issues={[bugIssue, featureIssue]} onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: "bug" }));

    expect(screen.getByText("Bug issue")).toBeInTheDocument();
    expect(screen.queryByText("Feature issue")).not.toBeInTheDocument();
  });

  it("should deselect label filter on second click", async () => {
    const user = userEvent.setup();
    const bugIssue = makeIssue({ number: 1, title: "Bug issue", state: "open", labels: ["bug"] });
    const featureIssue = makeIssue({
      number: 2,
      title: "Feature issue",
      state: "open",
      labels: ["feature"],
    });

    render(<Issues issues={[bugIssue, featureIssue]} onOpen={onOpen} />);

    const bugButton = screen.getByRole("button", { name: "bug" });
    await user.click(bugButton);
    await user.click(bugButton);

    expect(screen.getByText("Bug issue")).toBeInTheDocument();
    expect(screen.getByText("Feature issue")).toBeInTheDocument();
  });

  it("should combine repo + label + search + tab filters", async () => {
    const user = userEvent.setup();
    const issue1 = makeIssue({
      number: 1,
      title: "Repo1 bug open",
      state: "open",
      repoId: "repo-1",
      labels: ["bug"],
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Repo2 bug open",
      state: "open",
      repoId: "repo-2",
      labels: ["bug"],
    });
    const issue3 = makeIssue({
      number: 3,
      title: "Repo1 feature open",
      state: "open",
      repoId: "repo-1",
      labels: ["feature"],
    });
    const issue4 = makeIssue({
      number: 4,
      title: "Repo1 bug closed",
      state: "closed",
      repoId: "repo-1",
      labels: ["bug"],
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[issue1, issue2, issue3, issue4]} onOpen={onOpen} />);

    // Filter by repo-1
    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    await userEvent.selectOptions(select, "repo-1");

    // Filter by label "bug"
    await user.click(screen.getByRole("button", { name: "bug" }));

    // Search for "open"
    await user.type(screen.getByPlaceholderText("Filter issues..."), "open");

    // Only issue1 matches: repo-1, bug label, "open" in title, open tab
    expect(screen.getByText("Repo1 bug open")).toBeInTheDocument();
    expect(screen.queryByText("Repo2 bug open")).not.toBeInTheDocument();
    expect(screen.queryByText("Repo1 feature open")).not.toBeInTheDocument();
    expect(screen.queryByText("Repo1 bug closed")).not.toBeInTheDocument();
  });

  it("should reset stale repo/label filters after data changes", async () => {
    const issue1 = makeIssue({
      number: 1,
      title: "Issue repo1",
      repoId: "repo-1",
      labels: ["bug"],
      state: "open",
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Issue repo2",
      repoId: "repo-2",
      labels: ["feature"],
      state: "open",
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    const { rerender } = render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    // Select repo-2 and label "feature"
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: /filter by repo/i }),
      "repo-2",
    );
    await userEvent.click(screen.getByRole("button", { name: "feature" }));

    expect(screen.getByText("Issue repo2")).toBeInTheDocument();
    expect(screen.queryByText("Issue repo1")).not.toBeInTheDocument();

    // Rerender with data that no longer includes repo-2 or "feature" label
    const issue3 = makeIssue({
      number: 3,
      title: "Issue repo1 new",
      repoId: "repo-1",
      labels: ["docs"],
      state: "open",
    });
    mockUseQuery.mockReturnValue({ data: [makeRepo()] });
    rerender(<Issues issues={[issue1, issue3]} onOpen={onOpen} />);

    // Filters should reset — all items visible again
    expect(screen.getByText("Issue repo1")).toBeInTheDocument();
    expect(screen.getByText("Issue repo1 new")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: /filter by repo/i })).toBeNull();
  });

  it("should show only labels from the selected repo", async () => {
    const issue1 = makeIssue({
      number: 1,
      title: "Issue1",
      repoId: "repo-1",
      labels: ["bug"],
      state: "open",
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Issue2",
      repoId: "repo-2",
      labels: ["feature"],
      state: "open",
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    const labelGroup = screen.getByRole("group", { name: /filter by label/i });
    expect(within(labelGroup).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(labelGroup).getByRole("button", { name: "feature" })).toBeInTheDocument();

    // Select repo-1: only "bug" label should remain
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: /filter by repo/i }),
      "repo-1",
    );
    expect(within(labelGroup).getByRole("button", { name: "bug" })).toBeInTheDocument();
    expect(within(labelGroup).queryByRole("button", { name: "feature" })).not.toBeInTheDocument();
  });

  it("should only show LABEL_VISIBLE_LIMIT labels when more exist", () => {
    const labels = makeLabels(12);
    const issue = makeIssue({ number: 10, title: "Many labels issue", state: "open", labels });

    render(<Issues issues={[issue]} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by label/i });

    for (let i = 1; i <= 8; i++) {
      expect(
        within(group).getByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).toBeInTheDocument();
    }
    for (let i = 9; i <= 12; i++) {
      expect(
        within(group).queryByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).not.toBeInTheDocument();
    }
    expect(screen.getByTestId("label-filter-toggle")).toHaveTextContent("+4 more");
  });

  it("should show all labels when toggle is clicked", async () => {
    const user = userEvent.setup();
    const labels = makeLabels(12);
    const issue = makeIssue({ number: 10, title: "Many labels issue", state: "open", labels });

    render(<Issues issues={[issue]} onOpen={onOpen} />);

    const toggle = screen.getByTestId("label-filter-toggle");
    await user.click(toggle);

    const group = screen.getByRole("group", { name: /filter by label/i });
    for (let i = 1; i <= 12; i++) {
      expect(
        within(group).getByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).toBeInTheDocument();
    }
    expect(toggle).toHaveTextContent("Show less");
    expect(toggle).toHaveAttribute("aria-expanded", "true");
  });

  it("should collapse labels when Show less is clicked", async () => {
    const user = userEvent.setup();
    const labels = makeLabels(12);
    const issue = makeIssue({ number: 10, title: "Many labels issue", state: "open", labels });

    render(<Issues issues={[issue]} onOpen={onOpen} />);

    const toggle = screen.getByTestId("label-filter-toggle");
    await user.click(toggle);
    await user.click(toggle);

    const group = screen.getByRole("group", { name: /filter by label/i });
    for (let i = 1; i <= 8; i++) {
      expect(
        within(group).getByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).toBeInTheDocument();
    }
    for (let i = 9; i <= 12; i++) {
      expect(
        within(group).queryByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).not.toBeInTheDocument();
    }
    expect(toggle).toHaveTextContent("+4 more");
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  it("should not show toggle when labels are within limit", () => {
    const labels = makeLabels(LABEL_VISIBLE_LIMIT);
    const issue = makeIssue({ number: 10, title: "Exact limit issue", state: "open", labels });

    render(<Issues issues={[issue]} onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by label/i });
    for (let i = 1; i <= LABEL_VISIBLE_LIMIT; i++) {
      expect(
        within(group).getByRole("button", { name: `label-${String(i).padStart(2, "0")}` }),
      ).toBeInTheDocument();
    }
    expect(screen.queryByTestId("label-filter-toggle")).toBeNull();
  });

  it("should reset label expansion when repo filter changes", async () => {
    const user = userEvent.setup();
    const labels1 = makeLabels(12);
    const labels2 = makeLabels(10).map((l) => `repo2-${l}`);
    const issue1 = makeIssue({
      number: 1,
      title: "Repo1 issue",
      state: "open",
      repoId: "repo-1",
      labels: labels1,
    });
    const issue2 = makeIssue({
      number: 2,
      title: "Repo2 issue",
      state: "open",
      repoId: "repo-2",
      labels: labels2,
    });
    mockUseQuery.mockReturnValue({
      data: [
        makeRepo(),
        makeRepo({ id: "repo-2", org: "acme", name: "console", fullName: "acme/console" }),
      ],
    });

    render(<Issues issues={[issue1, issue2]} onOpen={onOpen} />);

    const toggle = screen.getByTestId("label-filter-toggle");
    await user.click(toggle);
    expect(toggle).toHaveTextContent("Show less");

    const select = screen.getByRole("combobox", { name: /filter by repo/i });
    await userEvent.selectOptions(select, "repo-2");

    const newToggle = screen.getByTestId("label-filter-toggle");
    expect(newToggle).not.toHaveTextContent("Show less");
    expect(newToggle).toHaveAttribute("aria-expanded", "false");
  });

  it("should keep selected label visible when collapsed beyond limit", async () => {
    const user = userEvent.setup();
    const labels = makeLabels(12);
    const issue = makeIssue({ number: 1, title: "Many labels", state: "open", labels });

    render(<Issues issues={[issue]} onOpen={onOpen} />);

    // Expand, select label-10 (beyond LABEL_VISIBLE_LIMIT), then collapse
    await user.click(screen.getByTestId("label-filter-toggle"));
    await user.click(screen.getByRole("button", { name: "label-10" }));
    await user.click(screen.getByTestId("label-filter-toggle"));

    // label-10 should still be visible even though list is collapsed
    const group = screen.getByRole("group", { name: /filter by label/i });
    const visibleLabelButtons = within(group)
      .getAllByRole("button")
      .filter((button) => /^label-/.test(button.textContent ?? ""));

    expect(visibleLabelButtons).toHaveLength(LABEL_VISIBLE_LIMIT);
    expect(within(group).getByRole("button", { name: "label-10" })).toBeInTheDocument();
    expect(within(group).queryByRole("button", { name: "label-08" })).not.toBeInTheDocument();
    expect(screen.getByTestId("label-filter-toggle")).toHaveTextContent("+4 more");
    expect(screen.getByRole("button", { name: "label-10" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
});
