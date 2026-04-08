import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Activity } from "../../lib/types";
import { ActivityFeed } from "./ActivityFeed";

function makeActivity(overrides: Partial<Activity> = {}): Activity {
  return {
    id: "act-1",
    activityType: "comment_added",
    actor: "alice",
    repoId: "org/repo",
    pullRequestId: "pr-1",
    issueId: null,
    message: "Some comment",
    isRead: false,
    createdAt: "2026-03-28T10:00:00Z",
    ...overrides,
  };
}

const commentActivity = makeActivity({ id: "act-c", activityType: "comment_added", message: "A comment" });
const mentionActivity = makeActivity({ id: "act-m", activityType: "comment_added", message: "Hey @alice check this out" });
const reviewActivity = makeActivity({ id: "act-r", activityType: "review_submitted", message: "Approved" });
const ciActivity = makeActivity({ id: "act-ci", activityType: "ci_completed", message: "CI passed" });
const prOpenedActivity = makeActivity({ id: "act-pr", activityType: "pr_opened", message: "Opened PR" });
const issueClosed = makeActivity({ id: "act-ic", activityType: "issue_closed", message: "Issue closed" });

const allActivities = [commentActivity, mentionActivity, reviewActivity, ciActivity, prOpenedActivity, issueClosed];

const onMarkAllRead = vi.fn();

beforeEach(() => {
  onMarkAllRead.mockClear();
});

describe("ActivityFeed", () => {
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

  it("should render SectionHead with filtered count when activities are provided", () => {
    render(<ActivityFeed activities={allActivities} onMarkAllRead={onMarkAllRead} />);

    expect(screen.getByText("Activity")).toBeInTheDocument();
    expect(screen.getByText("6")).toBeInTheDocument();
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
