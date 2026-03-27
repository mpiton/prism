import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useNotifications } from "./useNotifications";

vi.mock("../lib/tauri", () => ({
  onEvent: vi.fn(),
}));

import { onEvent } from "../lib/tauri";

describe("useNotifications", () => {
  let unlistenReviewRequest: Mock;
  let unlistenCiFailure: Mock;
  let unlistenPrApproved: Mock;

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenReviewRequest = vi.fn();
    unlistenCiFailure = vi.fn();
    unlistenPrApproved = vi.fn();

    (onEvent as Mock)
      .mockResolvedValueOnce(unlistenReviewRequest)
      .mockResolvedValueOnce(unlistenCiFailure)
      .mockResolvedValueOnce(unlistenPrApproved);
  });

  it("should capture review_request event", async () => {
    const { result } = renderHook(() => useNotifications());

    // Wait for listeners to register
    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "notification:review_request",
        expect.any(Function),
      );
    });

    // Simulate event
    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "notification:review_request",
    );
    const handler = call![1] as (payload: unknown) => void;

    act(() => {
      handler({ prNumber: 42, repo: "mpiton/prism", title: "Fix bug" });
    });

    expect(result.current.notifications).toHaveLength(1);
    const notif = result.current.notifications[0];
    expect(notif).toMatchObject({
      type: "review_request",
      payload: { prNumber: 42, repo: "mpiton/prism", title: "Fix bug" },
    });
    expect(notif?.id).toBeDefined();
    expect(notif?.timestamp).toBeDefined();
  });

  it("should capture ci_failure event", async () => {
    const { result } = renderHook(() => useNotifications());

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "notification:ci_failure",
        expect.any(Function),
      );
    });

    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "notification:ci_failure",
    );
    const handler = call![1] as (payload: unknown) => void;

    act(() => {
      handler({ prNumber: 10, repo: "mpiton/prism", check: "ci/build" });
    });

    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({
      type: "ci_failure",
      payload: { prNumber: 10, repo: "mpiton/prism", check: "ci/build" },
    });
  });

  it("should clear notification", async () => {
    const { result } = renderHook(() => useNotifications());

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledTimes(3);
    });

    // Add two notifications
    const reviewHandler = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "notification:review_request",
    )![1] as (payload: unknown) => void;

    const approvedHandler = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "notification:pr_approved",
    )![1] as (payload: unknown) => void;

    act(() => {
      reviewHandler({ prNumber: 1 });
      approvedHandler({ prNumber: 2 });
    });

    expect(result.current.notifications).toHaveLength(2);

    // Clear the first notification
    const firstNotif = result.current.notifications[0];
    expect(firstNotif).toBeDefined();
    const idToRemove = firstNotif!.id;
    act(() => {
      result.current.clearNotification(idToRemove);
    });

    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]?.id).not.toBe(idToRemove);
  });

  it("should call all unlisten functions on unmount", async () => {
    const { unmount } = renderHook(() => useNotifications());

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledTimes(3);
    });

    unmount();

    expect(unlistenReviewRequest).toHaveBeenCalledOnce();
    expect(unlistenCiFailure).toHaveBeenCalledOnce();
    expect(unlistenPrApproved).toHaveBeenCalledOnce();
  });

  it("should capture pr_approved event", async () => {
    const { result } = renderHook(() => useNotifications());

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "notification:pr_approved",
        expect.any(Function),
      );
    });

    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "notification:pr_approved",
    );
    const handler = call![1] as (payload: unknown) => void;

    act(() => {
      handler({ prNumber: 5, repo: "mpiton/prism" });
    });

    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0]).toMatchObject({
      type: "pr_approved",
      payload: { prNumber: 5, repo: "mpiton/prism" },
    });
  });
});
