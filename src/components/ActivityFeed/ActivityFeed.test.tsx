import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_RING } from "../../lib/a11y";
import type { Activity, Repo } from "../../lib/types/github";
import { ActivityFeed } from "./ActivityFeed";

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

function makeActivity(overrides: Partial<Activity> = {}): Activity {
  return {
    id: "act-1",
    activityType: "comment_added",
    actor: "alice",
    repoId: "repo-1",
    pullRequestId: "pr-1",
    issueId: null,
    message: "Some comment",
    isRead: false,
    createdAt: "2026-03-28T10:00:00Z",
    ...overrides,
  };
}

const commentActivity = makeActivity({
  id: "act-c",
  activityType: "comment_added",
  message: "A comment",
});
const mentionActivity = makeActivity({
  id: "act-m",
  activityType: "comment_added",
  message: "Hey @alice check this out",
});
const reviewActivity = makeActivity({
  id: "act-r",
  activityType: "review_submitted",
  message: "Approved",
});
const ciActivity = makeActivity({
  id: "act-ci",
  activityType: "ci_completed",
  message: "CI passed",
});
const prOpenedActivity = makeActivity({
  id: "act-pr",
  activityType: "pr_opened",
  message: "Opened PR",
});
const issueClosed = makeActivity({
  id: "act-ic",
  activityType: "issue_closed",
  message: "Issue closed",
});

const allActivities = [
  commentActivity,
  mentionActivity,
  reviewActivity,
  ciActivity,
  prOpenedActivity,
  issueClosed,
];

const onMarkAllRead = vi.fn();

beforeEach(() => {
  onMarkAllRead.mockClear();
  mockUseQuery.mockReturnValue({ data: [makeRepo()] });
});

describe("ActivityFeed", () => {
  it("should apply the focus-visible ring to the search input (WCAG 2.4.7)", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);
    const search = screen.getByRole("searchbox", { name: /filter activity/i });
    for (const token of FOCUS_RING.trim().split(/\s+/)) {
      expect(search).toHaveClass(token);
    }
  });

  it("should render all activities when no filter is selected", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    expect(screen.getAllByTestId("activity-item")).toHaveLength(6);
  });

  it("should filter by type when comment filter is clicked", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /comment/i }));

    expect(screen.getAllByTestId("activity-item")).toHaveLength(2);
  });

  it("should show all when All filter is selected after filtering", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /review/i }));
    await user.click(screen.getByRole("button", { name: /^all$/i }));

    expect(screen.getAllByTestId("activity-item")).toHaveLength(6);
  });

  it("should filter mentions when mention filter is clicked", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /mention/i }));

    expect(screen.getAllByTestId("activity-item")).toHaveLength(1);
  });

  it("should filter activities by actor, repo, and message", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    const input = screen.getByPlaceholderText("Filter activity...");

    await user.type(input, "approved");
    expect(screen.getAllByTestId("activity-item")).toHaveLength(1);
    expect(screen.getByText("Approved")).toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "org/repo");
    expect(screen.getAllByTestId("activity-item")).toHaveLength(6);

    await user.clear(input);
    await user.type(input, "alice");
    expect(screen.getAllByTestId("activity-item")).toHaveLength(6);
  });

  it("should combine text search with the type filter", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /comment/i }));
    await user.type(screen.getByPlaceholderText("Filter activity..."), "hey");

    expect(screen.getAllByTestId("activity-item")).toHaveLength(1);
    expect(screen.getByText(/hey @alice check this out/i)).toBeInTheDocument();
  });

  it("should mark all as read when button is clicked", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /mark all read/i }));

    expect(onMarkAllRead).toHaveBeenCalledOnce();
  });

  it("should show empty state when no activities match filter", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={[commentActivity]} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /ci/i }));

    expect(screen.getByText(/no activity/i)).toBeInTheDocument();
  });

  it("should render list skeletons while loading", () => {
    render(<ActivityFeed activities={[]} isLoading onMarkAllRead={onMarkAllRead} />);

    expect(screen.getByTestId("activity-feed")).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByTestId("activity-item-skeleton")).toHaveLength(4);
    expect(screen.queryByText(/no activity/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /filter by type/i })).not.toBeInTheDocument();
  });

  it("should render SectionHead with filtered count when activities are provided", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    expect(screen.getByText("Activity")).toBeInTheDocument();
    expect(screen.getByText("6")).toBeInTheDocument();
  });

  it("should prefer an explicit header count over the visible activity count", () => {
    render(
      <ActivityFeed
        activities={[commentActivity]}
        headerCount={149}
        onMarkAllRead={onMarkAllRead}
      />,
    );

    expect(screen.getByRole("heading", { name: "Activity 149" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Activity 1" })).not.toBeInTheDocument();
  });

  it("should hide section header when hideHeader is true", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} hideHeader />);

    expect(screen.queryByText("Activity")).not.toBeInTheDocument();
  });

  it("should keep filter buttons at the minimum touch target size", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    const filterGroup = screen.getByRole("group", { name: /filter by type/i });
    const buttons = within(filterGroup).getAllByRole("button");

    for (const button of buttons) {
      expect(button).toHaveClass("min-h-11", "min-w-11");
    }
  });

  it("should keep the mark all read action at the minimum touch target height", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    expect(screen.getByRole("button", { name: /mark all read/i })).toHaveClass("min-h-11");
  });

  it("should render the ci filter label in uppercase", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    expect(screen.getByRole("button", { name: "CI" })).toBeInTheDocument();
  });

  it("should set aria-pressed correctly when filter button is clicked", async () => {
    const user = userEvent.setup();
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    await user.click(screen.getByRole("button", { name: /review/i }));

    const filterGroup = screen.getByRole("group", { name: /filter by type/i });
    const pressedButtons = Array.from(filterGroup.querySelectorAll('[aria-pressed="true"]'));
    expect(pressedButtons).toHaveLength(1);
    expect(pressedButtons[0]).toHaveTextContent("review");
  });
});
