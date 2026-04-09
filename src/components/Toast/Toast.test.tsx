import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Toast } from "./Toast";

vi.mock("../../hooks/useNotifications", () => ({
  useNotifications: vi.fn(),
}));

vi.mock("../../stores/dashboard", () => ({
  useDashboardStore: vi.fn(),
}));

import { useNotifications } from "../../hooks/useNotifications";
import { useDashboardStore } from "../../stores/dashboard";
import type { Notification } from "../../hooks/useNotifications";

function makeNotification(overrides: Partial<Notification> = {}): Notification {
  return {
    id: "notif-1",
    type: "review_request",
    payload: { prNumber: 42, repo: "mpiton/prism", title: "Fix bug" },
    timestamp: Date.now(),
    ...overrides,
  };
}

describe("Toast", () => {
  const mockClearNotification = vi.fn();
  const mockSetView = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();

    (useNotifications as Mock).mockReturnValue({
      notifications: [],
      clearNotification: mockClearNotification,
    });

    (useDashboardStore as unknown as Mock).mockReturnValue(mockSetView);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("should show toast on review_request event", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [makeNotification()],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("Review Request")).toBeInTheDocument();
    expect(screen.getByTestId("toast-icon-review_request")).toBeInTheDocument();
  });

  it("should show toast on ci_failure event", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [
        makeNotification({
          id: "notif-ci",
          type: "ci_failure",
          payload: { prNumber: 10, repo: "mpiton/prism", check: "ci/build" },
        }),
      ],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("CI Failure")).toBeInTheDocument();
    expect(screen.getByTestId("toast-icon-ci_failure")).toBeInTheDocument();
  });

  it("should show toast on pr_approved event", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [
        makeNotification({
          id: "notif-approved",
          type: "pr_approved",
          payload: { prNumber: 5, repo: "mpiton/prism" },
        }),
      ],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("PR Approved")).toBeInTheDocument();
    expect(screen.getByTestId("toast-icon-pr_approved")).toBeInTheDocument();
  });

  it("should auto-dismiss after 5s", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [makeNotification()],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("Review Request")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(mockClearNotification).toHaveBeenCalledWith("notif-1");
  });

  it("should not dismiss after unmount", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [makeNotification()],
      clearNotification: mockClearNotification,
    });

    const { unmount } = render(<Toast />);
    unmount();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(mockClearNotification).not.toHaveBeenCalled();
  });

  it("should display payload summary in toast", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [makeNotification()],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("#42 Fix bug")).toBeInTheDocument();
  });

  it("should navigate on click", async () => {
    vi.useRealTimers();

    (useNotifications as Mock).mockReturnValue({
      notifications: [makeNotification()],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    const toast = screen.getByRole("button");
    const user = userEvent.setup();
    await user.click(toast);

    expect(mockSetView).toHaveBeenCalledWith("reviews");
    expect(mockClearNotification).toHaveBeenCalledWith("notif-1");
  });

  it("should navigate to mine on ci_failure click", async () => {
    vi.useRealTimers();

    (useNotifications as Mock).mockReturnValue({
      notifications: [
        makeNotification({ id: "notif-ci", type: "ci_failure" }),
      ],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    const toast = screen.getByRole("button");
    const user = userEvent.setup();
    await user.click(toast);

    expect(mockSetView).toHaveBeenCalledWith("mine");
    expect(mockClearNotification).toHaveBeenCalledWith("notif-ci");
  });

  it("should render multiple toasts", () => {
    (useNotifications as Mock).mockReturnValue({
      notifications: [
        makeNotification({ id: "n1", type: "review_request" }),
        makeNotification({ id: "n2", type: "ci_failure" }),
        makeNotification({ id: "n3", type: "pr_approved" }),
      ],
      clearNotification: mockClearNotification,
    });

    render(<Toast />);

    expect(screen.getByText("Review Request")).toBeInTheDocument();
    expect(screen.getByText("CI Failure")).toBeInTheDocument();
    expect(screen.getByText("PR Approved")).toBeInTheDocument();
  });

  it("should render nothing when no notifications", () => {
    const { container } = render(<Toast />);

    expect(container.querySelector("[data-testid='toast-container']")?.children).toHaveLength(0);
  });
});
