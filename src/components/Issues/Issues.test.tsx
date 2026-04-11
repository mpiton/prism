import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_RING } from "../../lib/a11y";
import type { Issue, Repo } from "../../lib/types/github";
import { Issues } from "./Issues";

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

const openIssue1 = makeIssue({ number: 1, title: "Open issue one", state: "open" });
const openIssue2 = makeIssue({ number: 2, title: "Open issue two", state: "open" });
const closedIssue1 = makeIssue({ number: 3, title: "Closed issue one", state: "closed" });
const closedIssue2 = makeIssue({ number: 4, title: "Closed issue two", state: "closed" });

const allIssues = [openIssue1, openIssue2, closedIssue1, closedIssue2];

const onOpen = vi.fn();

beforeEach(() => {
  onOpen.mockClear();
  mockUseQuery.mockReturnValue({ data: [makeRepo()] });
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

    expect(screen.getByText("org-a/shared")).toBeInTheDocument();
    expect(screen.getByText("org-b/shared")).toBeInTheDocument();
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

  it("should be wrapped in React.memo to bail out of re-renders on stable props", () => {
    // React.memo sets `$$typeof` to Symbol.for("react.memo") on the exported value.
    // This structural check guarantees the optimization cannot be accidentally removed.
    const memoSymbol = (Issues as unknown as { readonly $$typeof?: symbol }).$$typeof;
    expect(memoSymbol).toBe(Symbol.for("react.memo"));
  });
});
