import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_RING } from "../../lib/a11y";
import type { GithubNotification } from "../../lib/types/github";
import { Notifications } from "./Notifications";

const { mockUseQuery } = vi.hoisted(() => ({ mockUseQuery: vi.fn() }));

vi.mock("@tanstack/react-query", async () => {
  const actual = await vi.importActual("@tanstack/react-query");
  return {
    ...actual,
    useQuery: mockUseQuery,
  };
});

function makeNotification(overrides: Partial<GithubNotification> = {}): GithubNotification {
  return {
    id: "1",
    repo: "octocat/Hello-World",
    title: "Fix the bug",
    notificationType: "pullRequest",
    reason: "review_requested",
    unread: true,
    updatedAt: "2026-04-01T10:00:00Z",
    url: "https://github.com/octocat/Hello-World/pull/42",
    ...overrides,
  };
}

const unreadNotif = makeNotification({ id: "1", title: "Unread PR", unread: true });
const readNotif = makeNotification({ id: "2", title: "Read PR", unread: false });
const issueNotif = makeNotification({
  id: "3",
  title: "Crash",
  notificationType: "issue",
  unread: true,
  reason: "mention",
  repo: "octocat/other-repo",
});

const onOpen = vi.fn();

function mockQuerySuccess(data: GithubNotification[]) {
  mockUseQuery.mockReturnValue({
    data,
    isLoading: false,
    isError: false,
    error: null,
  });
}

function mockQueryLoading() {
  mockUseQuery.mockReturnValue({
    data: undefined,
    isLoading: true,
    isError: false,
    error: null,
  });
}

function mockQueryError(message: string) {
  mockUseQuery.mockReturnValue({
    data: undefined,
    isLoading: false,
    isError: true,
    error: new Error(message),
  });
}

beforeEach(() => {
  onOpen.mockClear();
  mockUseQuery.mockReset();
});

describe("Notifications", () => {
  it("applies the focus-visible ring to the search input", () => {
    mockQuerySuccess([unreadNotif, readNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);
    const search = screen.getByRole("searchbox", { name: /filter notifications/i });
    for (const token of FOCUS_RING.trim().split(/\s+/)) {
      expect(search).toHaveClass(token);
    }
  });

  it("shows only unread notifications on the unread tab by default", () => {
    mockQuerySuccess([unreadNotif, readNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByText("Unread PR")).toBeInTheDocument();
    expect(screen.getByText("Crash")).toBeInTheDocument();
    expect(screen.queryByText("Read PR")).not.toBeInTheDocument();
  });

  it("switches to the all tab and shows read notifications", async () => {
    const user = userEvent.setup();
    mockQuerySuccess([unreadNotif, readNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);

    await user.click(screen.getByRole("button", { name: /^all/i }));

    expect(screen.getByText("Unread PR")).toBeInTheDocument();
    expect(screen.getByText("Read PR")).toBeInTheDocument();
    expect(screen.getByText("Crash")).toBeInTheDocument();
  });

  it("shows correct counts on each tab", () => {
    mockQuerySuccess([unreadNotif, readNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);

    const group = screen.getByRole("group", { name: /filter by state/i });
    const buttons = within(group).getAllByRole("button");

    expect(buttons[0]).toHaveTextContent("2"); // unread: 2
    expect(buttons[1]).toHaveTextContent("3"); // all: 3
  });

  it("filters notifications by title and repo", async () => {
    const user = userEvent.setup();
    mockQuerySuccess([unreadNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);

    const input = screen.getByPlaceholderText(/filter notifications/i);

    await user.type(input, "crash");
    expect(screen.getByText("Crash")).toBeInTheDocument();
    expect(screen.queryByText("Unread PR")).not.toBeInTheDocument();

    await user.clear(input);
    await user.type(input, "other-repo");
    expect(screen.getByText("Crash")).toBeInTheDocument();
    expect(screen.queryByText("Unread PR")).not.toBeInTheDocument();
  });

  it("renders skeletons while loading", () => {
    mockQueryLoading();
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByTestId("notifications")).toHaveAttribute("aria-busy", "true");
    expect(screen.getAllByTestId("notification-card-skeleton")).toHaveLength(3);
  });

  it("shows an empty state when there are no unread notifications", () => {
    mockQuerySuccess([readNotif]);
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByText(/no unread notifications/i)).toBeInTheDocument();
  });

  it("shows an empty state when the result list is empty", () => {
    mockQuerySuccess([]);
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByText(/no unread notifications/i)).toBeInTheDocument();
  });

  it("renders an error banner when fetching fails", () => {
    mockQueryError("network down");
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByRole("alert")).toHaveTextContent(/network down/i);
  });

  it("renders SectionHead with title and count", () => {
    mockQuerySuccess([unreadNotif, readNotif, issueNotif]);
    render(<Notifications onOpen={onOpen} />);

    expect(screen.getByText("Notifications")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("forwards onOpen to the card click handler", async () => {
    const user = userEvent.setup();
    mockQuerySuccess([unreadNotif]);
    render(<Notifications onOpen={onOpen} />);

    await user.click(screen.getByText("Unread PR"));

    expect(onOpen).toHaveBeenCalledWith(unreadNotif.url);
  });
});
