import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { render, screen, act } from "@testing-library/react";

// ── Hoisted mocks (available before vi.mock factories) ───────────

const {
  mockOpen,
  mockWrite,
  mockOnData,
  mockLoadAddon,
  mockDispose,
  mockFit,
  MockTerminal,
  MockFitAddon,
  MockWebLinksAddon,
} = vi.hoisted(() => {
  const _mockOpen = vi.fn();
  const _mockWrite = vi.fn();
  const _mockOnData = vi.fn();
  const _mockLoadAddon = vi.fn();
  const _mockDispose = vi.fn();
  const _mockFit = vi.fn();
  const _mockPropose = vi.fn().mockReturnValue({ cols: 120, rows: 40 });

  return {
    mockOpen: _mockOpen,
    mockWrite: _mockWrite,
    mockOnData: _mockOnData,
    mockLoadAddon: _mockLoadAddon,
    mockDispose: _mockDispose,
    mockFit: _mockFit,
    mockPropose: _mockPropose,
    MockTerminal: vi.fn(function (this: Record<string, unknown>) {
      this.open = _mockOpen;
      this.write = _mockWrite;
      this.onData = _mockOnData;
      this.loadAddon = _mockLoadAddon;
      this.dispose = _mockDispose;
      this.cols = 80;
      this.rows = 24;
    }),
    MockFitAddon: vi.fn(function (this: Record<string, unknown>) {
      this.fit = _mockFit;
      this.proposeDimensions = _mockPropose;
      this.dispose = vi.fn();
    }),
    MockWebLinksAddon: vi.fn(function (this: Record<string, unknown>) {
      this.dispose = vi.fn();
    }),
  };
});

// ── Module mocks ─────────────────────────────────────────────────

vi.mock("@xterm/xterm", () => ({ Terminal: MockTerminal }));
vi.mock("@xterm/addon-fit", () => ({ FitAddon: MockFitAddon }));
vi.mock("@xterm/addon-web-links", () => ({ WebLinksAddon: MockWebLinksAddon }));

vi.mock("../../lib/tauri", () => ({
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  ptyResize: vi.fn().mockResolvedValue(undefined),
  onEvent: vi.fn(),
}));

import { Terminal } from "./Terminal";
import { ptyWrite, ptyResize, onEvent } from "../../lib/tauri";

describe("Terminal", () => {
  let unlistenStdout: Mock;
  let idleCallback: IdleRequestCallback | null;
  let cancelIdleCallbackMock: Mock;

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenStdout = vi.fn();
    (onEvent as Mock).mockResolvedValue(unlistenStdout);
    idleCallback = null;
    cancelIdleCallbackMock = vi.fn();
    vi.stubGlobal(
      "requestIdleCallback",
      vi.fn((callback: IdleRequestCallback) => {
        idleCallback = callback;
        return 1;
      }),
    );
    vi.stubGlobal("cancelIdleCallback", cancelIdleCallbackMock);
  });

  function flushIdleCallback() {
    expect(idleCallback).not.toBeNull();
    act(() => {
      idleCallback?.({
        didTimeout: false,
        timeRemaining: () => 50,
      } as IdleDeadline);
    });
  }

  it("should initialize xterm on mount", () => {
    render(<Terminal ptyId="ws-1" />);

    expect(onEvent).toHaveBeenCalledWith(
      "workspace:stdout",
      expect.any(Function),
    );
    flushIdleCallback();

    expect(MockTerminal).toHaveBeenCalledWith(
      expect.objectContaining({
        cursorBlink: true,
        theme: expect.objectContaining({
          background: "#0a0a09",
          foreground: "#d4d0c8",
          cursor: "#c8ff00",
        }),
      }),
    );
    expect(mockOpen).toHaveBeenCalledOnce();
    expect(mockLoadAddon).toHaveBeenCalledTimes(2); // FitAddon + WebLinksAddon
    expect(mockFit).toHaveBeenCalledOnce();
    expect(screen.getByTestId("terminal-ws-1")).toBeInTheDocument();
  });

  it("should buffer stdout received before terminal initialization", async () => {
    render(<Terminal ptyId="ws-1" />);

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:stdout",
        expect.any(Function),
      );
    });

    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "workspace:stdout",
    );
    expect(call).toBeDefined();
    const handler = call![1] as (payload: { workspaceId: string; data: string }) => void;

    act(() => {
      handler({ workspaceId: "ws-1", data: "boot output" });
    });

    expect(mockWrite).not.toHaveBeenCalled();

    flushIdleCallback();

    expect(mockWrite).toHaveBeenCalledWith("boot output");
  });

  it("should write data from stdout event", async () => {
    render(<Terminal ptyId="ws-1" />);
    flushIdleCallback();

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:stdout",
        expect.any(Function),
      );
    });

    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "workspace:stdout",
    );
    expect(call).toBeDefined();
    const handler = call![1] as (payload: { workspaceId: string; data: string }) => void;

    act(() => {
      handler({ workspaceId: "ws-1", data: "hello world" });
    });

    expect(mockWrite).toHaveBeenCalledWith("hello world");
  });

  it("should not write stdout for different ptyId", async () => {
    render(<Terminal ptyId="ws-1" />);
    flushIdleCallback();

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        "workspace:stdout",
        expect.any(Function),
      );
    });

    const call = (onEvent as Mock).mock.calls.find(
      (c: unknown[]) => c[0] === "workspace:stdout",
    );
    expect(call).toBeDefined();
    const handler = call![1] as (payload: { workspaceId: string; data: string }) => void;

    act(() => {
      handler({ workspaceId: "ws-other", data: "wrong terminal" });
    });

    expect(mockWrite).not.toHaveBeenCalled();
  });

  it("should call pty_write on keypress", () => {
    render(<Terminal ptyId="ws-1" />);
    flushIdleCallback();

    expect(mockOnData).toHaveBeenCalledOnce();
    const calls = (mockOnData as Mock).mock.calls;
    expect(calls[0]).toBeDefined();
    const onDataCallback = calls[0]![0] as (data: string) => void;

    onDataCallback("a");

    expect(ptyWrite).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      data: "a",
    });
  });

  it("should call pty_resize on container resize", async () => {
    let resizeCallback: ResizeObserverCallback | null = null;
    const mockObserve = vi.fn();
    const mockDisconnect = vi.fn();

    const MockResizeObserver = vi.fn(function (
      this: { observe: Mock; disconnect: Mock; unobserve: Mock },
      cb: ResizeObserverCallback,
    ) {
      resizeCallback = cb;
      this.observe = mockObserve;
      this.disconnect = mockDisconnect;
      this.unobserve = vi.fn();
    });

    vi.stubGlobal("ResizeObserver", MockResizeObserver);

    render(<Terminal ptyId="ws-1" />);
    flushIdleCallback();

    expect(mockObserve).toHaveBeenCalledOnce();

    act(() => {
      resizeCallback!([], {} as ResizeObserver);
    });

    expect(mockFit).toHaveBeenCalled();
    expect(ptyResize).toHaveBeenCalledWith({
      workspaceId: "ws-1",
      cols: 120,
      rows: 40,
    });

    vi.unstubAllGlobals();
  });

  it("should cleanup on unmount", async () => {
    const { unmount } = render(<Terminal ptyId="ws-1" />);
    flushIdleCallback();

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledOnce();
    });

    unmount();

    // dispose is synchronous, unlisten is deferred via .then()
    expect(mockDispose).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(unlistenStdout).toHaveBeenCalledOnce();
    });
  });

  it("should cancel deferred initialization on unmount before idle callback", () => {
    const { unmount } = render(<Terminal ptyId="ws-1" />);

    unmount();

    expect(cancelIdleCallbackMock).toHaveBeenCalledWith(1);
    expect(MockTerminal).not.toHaveBeenCalled();
  });

  it("should unlisten stdout on unmount before idle callback", async () => {
    const { unmount } = render(<Terminal ptyId="ws-1" />);

    unmount();

    await vi.waitFor(() => {
      expect(unlistenStdout).toHaveBeenCalledOnce();
    });
  });
});
