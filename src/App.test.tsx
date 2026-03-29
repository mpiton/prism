import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { useDashboardStore } from "./stores/dashboard";

function renderApp() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );
}

describe("App layout", () => {
  beforeEach(() => {
    useDashboardStore.setState({
      currentView: "overview",
      activeFilters: {},
    });
  });

  it("should render sidebar", () => {
    renderApp();
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
  });

  it("should render main content area", () => {
    renderApp();
    expect(screen.getByRole("main")).toBeInTheDocument();
  });

  it("should switch views based on store", () => {
    useDashboardStore.setState({ currentView: "mine" });
    renderApp();
    expect(screen.getByTestId("my-prs")).toBeInTheDocument();

    act(() => {
      useDashboardStore.setState({ currentView: "issues" });
    });
    expect(screen.getByTestId("issues")).toBeInTheDocument();
    expect(screen.queryByTestId("my-prs")).not.toBeInTheDocument();
  });

  it("should render overview for default view", () => {
    renderApp();
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("should render my-prs view", () => {
    useDashboardStore.setState({ currentView: "mine" });
    renderApp();
    expect(screen.getByTestId("my-prs")).toBeInTheDocument();
  });

  it("should render issues view", () => {
    useDashboardStore.setState({ currentView: "issues" });
    renderApp();
    expect(screen.getByTestId("issues")).toBeInTheDocument();
  });

  it("should render activity feed view", () => {
    useDashboardStore.setState({ currentView: "feed" });
    renderApp();
    expect(screen.getByTestId("activity-feed")).toBeInTheDocument();
  });

  it("should render settings view", () => {
    useDashboardStore.setState({ currentView: "settings" });
    renderApp();
    expect(screen.getByTestId("settings")).toBeInTheDocument();
  });

  it("should render workspace in workspace mode", () => {
    useDashboardStore.setState({ currentView: "workspaces" });
    renderApp();
    expect(screen.getByTestId("workspace")).toBeInTheDocument();
  });

  it("should keep sidebar visible in workspace mode", () => {
    useDashboardStore.setState({ currentView: "workspaces" });
    renderApp();
    expect(screen.getByTestId("sidebar")).toBeInTheDocument();
    expect(screen.getByTestId("workspace")).toBeInTheDocument();
  });
});

function fireKey(key: string, opts: Partial<KeyboardEventInit> = {}): void {
  document.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true, ...opts }),
  );
}

describe("App keyboard shortcuts", () => {
  const items = [
    { url: "https://github.com/org/repo/pull/1" },
    { url: "https://github.com/org/repo/pull/2" },
    { url: "https://github.com/org/repo/pull/3" },
  ];

  let openSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    // Use "feed" view — it has no useRegisterNavigableItems hook,
    // so pre-seeded navigableItems are preserved for keyboard tests.
    useDashboardStore.setState({
      currentView: "feed",
      activeFilters: {},
      selectedIndex: -1,
      navigableItems: items,
    });
    openSpy = vi.spyOn(window, "open").mockImplementation(() => null);
  });

  afterEach(() => {
    openSpy.mockRestore();
  });

  it("should navigate list with j/k", () => {
    renderApp();

    act(() => fireKey("j"));
    expect(useDashboardStore.getState().selectedIndex).toBe(0);

    act(() => fireKey("j"));
    expect(useDashboardStore.getState().selectedIndex).toBe(1);

    act(() => fireKey("k"));
    expect(useDashboardStore.getState().selectedIndex).toBe(0);
  });

  it("should open item with Enter", () => {
    useDashboardStore.setState({ selectedIndex: 1 });
    renderApp();

    act(() => fireKey("Enter"));
    expect(openSpy).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/2",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("should not open when no item is selected", () => {
    renderApp();

    act(() => fireKey("Enter"));
    expect(openSpy).not.toHaveBeenCalled();
  });

  it("should return to overview on Escape", () => {
    renderApp();

    act(() => fireKey("Escape"));
    expect(useDashboardStore.getState().currentView).toBe("overview");
  });

  it("should not crash when switching workspace with Ctrl+number", () => {
    renderApp();

    act(() => fireKey("1", { ctrlKey: true }));
    act(() => fireKey("2", { ctrlKey: true }));
    act(() => fireKey("3", { ctrlKey: true }));
    expect(screen.getByRole("main")).toBeInTheDocument();
  });

  it("should not capture keys when terminal is focused", () => {
    renderApp();

    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    try {
      textarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "j", bubbles: true }),
      );
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    } finally {
      document.body.removeChild(textarea);
    }
  });

  it("should reset selection when navigating to a different view", () => {
    useDashboardStore.setState({ selectedIndex: 2 });
    renderApp();

    act(() => fireKey("Escape"));
    expect(useDashboardStore.getState().selectedIndex).toBe(-1);
  });
});
