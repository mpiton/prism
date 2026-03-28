import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Activity } from "../../lib/types";
import { ActivityItem } from "./ActivityItem";

function makeActivity(overrides: Partial<Activity> = {}): Activity {
  return {
    id: "act-1",
    activityType: "comment_added",
    actor: "alice",
    repoId: "org/repo",
    pullRequestId: "pr-42",
    issueId: null,
    message: "Looks good, just one small nit on the error handling path",
    createdAt: "2026-03-28T10:00:00Z",
    ...overrides,
  };
}

describe("ActivityItem", () => {
  it("should render icon when activity type is review_submitted", () => {
    render(<ActivityItem activity={makeActivity({ activityType: "review_submitted" })} />);

    expect(screen.getByTestId("activity-icon")).toBeInTheDocument();
  });

  it("should render actor in bold when actor is provided", () => {
    render(<ActivityItem activity={makeActivity({ actor: "bob" })} />);

    const actor = screen.getByTestId("activity-actor");
    expect(actor).toHaveTextContent("bob");
    expect(actor.className).toContain("font-bold");
  });

  it("should render repo name when repoId is set", () => {
    render(<ActivityItem activity={makeActivity({ repoId: "my-org/my-repo" })} />);

    expect(screen.getByText("my-org/my-repo")).toBeInTheDocument();
  });

  it("should render relative time when createdAt is provided", () => {
    render(<ActivityItem activity={makeActivity()} />);

    expect(screen.getByTestId("activity-time")).toBeInTheDocument();
  });

  it("should truncate body when message exceeds 80 characters", () => {
    const longMessage = "A".repeat(200);
    render(<ActivityItem activity={makeActivity({ message: longMessage })} />);

    const body = screen.getByTestId("activity-body");
    expect(body).toHaveTextContent(`${"A".repeat(80)}…`);
    expect((body.textContent ?? "").length).toBe(81);
  });

  it("should show different icons when activity types differ", () => {
    const { rerender } = render(
      <ActivityItem activity={makeActivity({ activityType: "pr_opened" })} />,
    );
    const iconPrOpened = screen.getByTestId("activity-icon").textContent;

    rerender(
      <ActivityItem activity={makeActivity({ activityType: "ci_completed" })} />,
    );
    const iconCi = screen.getByTestId("activity-icon").textContent;

    expect(iconPrOpened).not.toBe(iconCi);
  });

  it("should render action text when activity type is pr_merged", () => {
    render(<ActivityItem activity={makeActivity({ activityType: "pr_merged" })} />);

    expect(screen.getByTestId("activity-action")).toHaveTextContent(/merged/i);
  });
});
