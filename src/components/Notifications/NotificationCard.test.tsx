import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { GithubNotification } from "../../lib/types/github";
import { NotificationCard } from "./NotificationCard";

function makeNotification(overrides: Partial<GithubNotification> = {}): GithubNotification {
  return {
    id: "1",
    repo: "octocat/Hello-World",
    title: "Fix the bug",
    notificationType: "pullRequest",
    reason: "review_requested",
    unread: true,
    updatedAt: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
    url: "https://github.com/octocat/Hello-World/pull/42",
    ...overrides,
  };
}

describe("NotificationCard", () => {
  it("renders the title, repo and humanized reason", () => {
    render(<NotificationCard data={makeNotification()} onOpen={vi.fn()} />);

    expect(screen.getByText("Fix the bug")).toBeInTheDocument();
    expect(screen.getByText("octocat/Hello-World")).toBeInTheDocument();
    expect(screen.getByText(/review requested/i)).toBeInTheDocument();
  });

  it("shows selected styling when keyboard-selected", () => {
    render(<NotificationCard data={makeNotification()} onOpen={vi.fn()} isSelected />);
    const card = screen.getByTestId("notification-card");
    expect(card).toHaveAttribute("data-selected", "true");
    expect(card).toHaveClass("border-accent", "ring-2", "ring-accent");
  });

  it("shows an unread indicator when unread is true", () => {
    render(<NotificationCard data={makeNotification({ unread: true })} onOpen={vi.fn()} />);
    expect(screen.getByTestId("unread-indicator")).toBeInTheDocument();
  });

  it("omits the unread indicator when notification is read", () => {
    render(<NotificationCard data={makeNotification({ unread: false })} onOpen={vi.fn()} />);
    expect(screen.queryByTestId("unread-indicator")).not.toBeInTheDocument();
  });

  it("calls onOpen with the URL on click", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(<NotificationCard data={makeNotification()} onOpen={onOpen} />);

    await user.click(screen.getByRole("link", { name: /fix the bug/i }));

    expect(onOpen).toHaveBeenCalledWith("https://github.com/octocat/Hello-World/pull/42");
  });

  it("renders different icon for issue vs pull request", () => {
    const { rerender } = render(
      <NotificationCard data={makeNotification({ notificationType: "issue" })} onOpen={vi.fn()} />,
    );
    const issueIcon = screen.getByTestId("notification-type-icon");
    expect(issueIcon).toHaveAttribute("data-type", "issue");

    rerender(
      <NotificationCard
        data={makeNotification({ notificationType: "pullRequest" })}
        onOpen={vi.fn()}
      />,
    );
    const prIcon = screen.getByTestId("notification-type-icon");
    expect(prIcon).toHaveAttribute("data-type", "pullRequest");
  });

  it("humanizes common reason values", () => {
    const reasons: Array<[string, RegExp]> = [
      ["mention", /mentioned/i],
      ["assign", /assigned/i],
      ["author", /author/i],
      ["subscribed", /subscribed/i],
      ["team_mention", /team mentioned/i],
      ["comment", /comment/i],
      ["state_change", /state/i],
    ];
    for (const [reason, label] of reasons) {
      const { unmount } = render(
        <NotificationCard data={makeNotification({ reason })} onOpen={vi.fn()} />,
      );
      expect(screen.getByText(label)).toBeInTheDocument();
      unmount();
    }
  });

  it("shows relative time for updatedAt", () => {
    render(
      <NotificationCard
        data={makeNotification({
          updatedAt: new Date(Date.now() - 5 * 60 * 1000).toISOString(),
        })}
        onOpen={vi.fn()}
      />,
    );
    expect(screen.getByTestId("time-ago")).toHaveTextContent(/5m/);
  });
});
