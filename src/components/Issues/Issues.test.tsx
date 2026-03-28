import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Issue } from "../../lib/types";
import { Issues } from "./Issues";

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
});

describe("Issues", () => {
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

  it("should show empty state when no open issues", () => {
    render(<Issues issues={[closedIssue1]} onOpen={onOpen} />);

    expect(screen.getByText(/no issues/i)).toBeInTheDocument();
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
});
