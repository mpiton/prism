import { type ReactElement } from "react";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, beforeEach, afterEach } from "vitest";

const ORIGINAL_USER_AGENT = window.navigator.userAgent;

function setUserAgent(userAgent: string): void {
  Object.defineProperty(window.navigator, "userAgent", {
    value: userAgent,
    configurable: true,
  });
}

const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
const MAC_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";

let KeyboardShortcuts: () => ReactElement;

beforeEach(async () => {
  const mod = await import("./KeyboardShortcuts");
  KeyboardShortcuts = mod.KeyboardShortcuts;
});

afterEach(() => {
  Object.defineProperty(window.navigator, "userAgent", {
    value: ORIGINAL_USER_AGENT,
    configurable: true,
  });
});

describe("KeyboardShortcuts", () => {
  it("should render section with settings-keyboard-shortcuts test id", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByTestId("settings-keyboard-shortcuts")).toBeInTheDocument();
  });

  it("should render the section header", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    const section = screen.getByTestId("settings-keyboard-shortcuts");
    expect(
      within(section).getByRole("heading", { name: /keyboard shortcuts/i }),
    ).toBeInTheDocument();
  });

  it("should render all three shortcut groups", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByRole("heading", { name: /global/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /list navigation/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /command palette/i })).toBeInTheDocument();
  });

  it("should display Ctrl label on non-Mac platforms", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    const ctrlTokens = screen.getAllByText("Ctrl");
    expect(ctrlTokens.length).toBeGreaterThan(0);
    expect(screen.queryByText("⌘")).toBeNull();
  });

  it("should display ⌘ symbol on Mac platforms", () => {
    setUserAgent(MAC_UA);
    render(<KeyboardShortcuts />);

    const cmdTokens = screen.getAllByText("⌘");
    expect(cmdTokens.length).toBeGreaterThan(0);
    expect(screen.queryByText("Ctrl")).toBeNull();
  });

  it("should display Open Command Palette action paired with K key", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByText(/open command palette/i)).toBeInTheDocument();
    expect(screen.getByText("K")).toBeInTheDocument();
  });

  it("should display list navigation shortcuts (j and k)", () => {
    setUserAgent(WINDOWS_UA);
    const { container } = render(<KeyboardShortcuts />);

    expect(screen.getByText(/navigate list down/i)).toBeInTheDocument();
    expect(screen.getByText(/navigate list up/i)).toBeInTheDocument();
    expect(screen.getByText("j")).toBeInTheDocument();
    // The "k" lowercase shortcut for navigate-up is distinct from "K" capital for Command Palette
    const kKbds = Array.from(container.querySelectorAll("kbd")).filter(
      (el) => el.textContent === "k",
    );
    expect(kKbds.length).toBe(1);
  });

  it("should display Back to overview shortcut", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByText(/back to overview/i)).toBeInTheDocument();
    // Esc appears at least twice (back to overview + close palette)
    const escTokens = screen.getAllByText("Esc");
    expect(escTokens.length).toBeGreaterThanOrEqual(2);
  });

  it("should display the three workspace-switching shortcuts", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByText(/switch to workspace 1/i)).toBeInTheDocument();
    expect(screen.getByText(/switch to workspace 2/i)).toBeInTheDocument();
    expect(screen.getByText(/switch to workspace 3/i)).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("should display Command Palette specific shortcuts", () => {
    setUserAgent(WINDOWS_UA);
    render(<KeyboardShortcuts />);

    expect(screen.getByText(/navigate results/i)).toBeInTheDocument();
    expect(screen.getByText(/open result in browser/i)).toBeInTheDocument();
    expect(screen.getByText(/close palette/i)).toBeInTheDocument();
  });

  it("should wrap every key token in a <kbd> element", () => {
    setUserAgent(WINDOWS_UA);
    const { container } = render(<KeyboardShortcuts />);

    const kbds = container.querySelectorAll("kbd");
    // 14 shortcuts total across 3 groups producing 22 <kbd> tokens:
    //   Global (5): Ctrl+K, Esc, Ctrl+1, Ctrl+2, Ctrl+3 = 9 kbds
    //   List Navigation (6): j, k, ↑/↓, Home/End, Enter, w = 8 kbds
    //   Command Palette (3): ↑/↓, Ctrl+Enter, Esc = 5 kbds
    // Locking the exact count catches accidental catalog drift.
    expect(kbds.length).toBe(22);
  });

  it("should render shortcuts inside a definition list structure", () => {
    setUserAgent(WINDOWS_UA);
    const { container } = render(<KeyboardShortcuts />);

    // Each group uses a <dl> with <dt> action / <dd> keys
    const dls = container.querySelectorAll("dl");
    expect(dls.length).toBe(3);
    const dts = container.querySelectorAll("dt");
    expect(dts.length).toBeGreaterThanOrEqual(13);
  });
});
