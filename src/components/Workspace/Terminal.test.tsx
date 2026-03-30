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

  beforeEach(() => {
    vi.clearAllMocks();
    unlistenStdout = vi.fn();
    (onEvent as Mock).mockResolvedValue(unlistenStdout);
  });

  it("should initialize xterm on mount", () => {
    render(<Terminal ptyId="ws-1" />);

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

  it("should write data from stdout event", async () => {
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
      handler({ workspaceId: "ws-1", data: "hello world" });
    });

    expect(mockWrite).toHaveBeenCalledWith("hello world");
  });

  it("should not write stdout for different ptyId", async () => {
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
      handler({ workspaceId: "ws-other", data: "wrong terminal" });
    });

    expect(mockWrite).not.toHaveBeenCalled();
  });

  it("should call pty_write on keypress", () => {
    render(<Terminal ptyId="ws-1" />);

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

    await vi.waitFor(() => {
      expect(onEvent).toHaveBeenCalledOnce();
    });

    unmount();

    expect(mockDispose).toHaveBeenCalledOnce();
    // unlisten is called via .then() — flush microtasks
    await vi.waitFor(() => {
      expect(unlistenStdout).toHaveBeenCalledOnce();
    });
  });
});
