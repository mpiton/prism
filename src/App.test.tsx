import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { useDashboardStore } from "./stores/dashboard";

vi.mock("./lib/open", () => ({
  openUrl: vi.fn(),
}));

vi.mock("./lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/tauri")>();
  return {
    ...actual,
    authGetStatus: vi.fn().mockResolvedValue({ connected: true, username: "test-user", error: null }),
  };
});

vi.mock("./hooks/useGitHubData", () => ({
  useGitHubData: vi.fn().mockReturnValue({
    dashboard: { syncedAt: "2026-03-28T10:00:00Z", reviewRequests: [], myPullRequests: [], assignedIssues: [], recentActivity: [], workspaces: [] },
    stats: { pendingReviews: 3, openPrs: 5, openIssues: 2, totalWorkspaces: 1, unreadActivity: 0 },
    isLoading: false,
    error: null,
    authExpired: false,
    forceSync: vi.fn(),
    isSyncing: false,
  }),
}));

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

  it("should render sidebar", async () => {
    renderApp();
    expect(await screen.findByTestId("sidebar")).toBeInTheDocument();
  });

  it("should render main content area", async () => {
    renderApp();
    expect(await screen.findByRole("main")).toBeInTheDocument();
  });

  it("should switch views based on store", async () => {
    useDashboardStore.setState({ currentView: "mine" });
    renderApp();
    expect(await screen.findByTestId("my-prs")).toBeInTheDocument();

    act(() => {
      useDashboardStore.setState({ currentView: "issues" });
    });
    expect(await screen.findByTestId("issues")).toBeInTheDocument();
    expect(screen.queryByTestId("my-prs")).not.toBeInTheDocument();
  });

  it("should render overview for default view", async () => {
    renderApp();
    expect(await screen.findByTestId("overview")).toBeInTheDocument();
  });

  it("should render my-prs view", async () => {
    useDashboardStore.setState({ currentView: "mine" });
    renderApp();
    expect(await screen.findByTestId("my-prs")).toBeInTheDocument();
  });

  it("should render issues view", async () => {
    useDashboardStore.setState({ currentView: "issues" });
    renderApp();
    expect(await screen.findByTestId("issues")).toBeInTheDocument();
  });

  it("should render activity feed view", async () => {
    useDashboardStore.setState({ currentView: "feed" });
    renderApp();
    expect(await screen.findByTestId("activity-feed")).toBeInTheDocument();
  });

  it("should render settings view", async () => {
    useDashboardStore.setState({ currentView: "settings" });
    renderApp();
    expect(await screen.findByTestId("settings")).toBeInTheDocument();
  });

  it("should render workspace in workspace mode", async () => {
    useDashboardStore.setState({ currentView: "workspaces" });
    renderApp();
    expect(await screen.findByTestId("workspace-view")).toBeInTheDocument();
  });

  it("should keep sidebar visible in workspace mode", async () => {
    useDashboardStore.setState({ currentView: "workspaces" });
    renderApp();
    expect(await screen.findByTestId("sidebar")).toBeInTheDocument();
    expect(await screen.findByTestId("workspace-view")).toBeInTheDocument();
  });

  it("should render stats bar on dashboard views", async () => {
    useDashboardStore.setState({ currentView: "reviews" });
    renderApp();
    expect(await screen.findByTestId("stats-bar")).toBeInTheDocument();
  });

  it("should not render stats bar in workspace mode", async () => {
    useDashboardStore.setState({ currentView: "workspaces" });
    renderApp();
    await screen.findByTestId("workspace-view");
    expect(screen.queryByTestId("stats-bar")).not.toBeInTheDocument();
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

  let mockedOpenUrl: ReturnType<typeof vi.fn>;

  beforeEach(async () => {
    // Use "feed" view — it has no useRegisterNavigableItems hook,
    // so pre-seeded navigableItems are preserved for keyboard tests.
    useDashboardStore.setState({
      currentView: "feed",
      activeFilters: {},
      selectedIndex: -1,
      navigableItems: items,
    });
    const { openUrl } = await import("./lib/open");
    mockedOpenUrl = vi.mocked(openUrl);
    mockedOpenUrl.mockClear();
  });

  it("should navigate list with j/k", async () => {
    renderApp();
    await screen.findByTestId("activity-feed");

    act(() => fireKey("j"));
    expect(useDashboardStore.getState().selectedIndex).toBe(0);

    act(() => fireKey("j"));
    expect(useDashboardStore.getState().selectedIndex).toBe(1);

    act(() => fireKey("k"));
    expect(useDashboardStore.getState().selectedIndex).toBe(0);
  });

  it("should open item with Enter", async () => {
    useDashboardStore.setState({ selectedIndex: 1 });
    renderApp();
    await screen.findByTestId("activity-feed");

    act(() => fireKey("Enter"));
    expect(mockedOpenUrl).toHaveBeenCalledWith(
      "https://github.com/org/repo/pull/2",
    );
  });

  it("should not open when no item is selected", async () => {
    renderApp();
    await screen.findByTestId("activity-feed");

    act(() => fireKey("Enter"));
    expect(mockedOpenUrl).not.toHaveBeenCalled();
  });

  it("should return to overview on Escape", async () => {
    renderApp();
    await screen.findByTestId("activity-feed");

    act(() => fireKey("Escape"));
    expect(useDashboardStore.getState().currentView).toBe("overview");
  });

  it("should not capture keys when terminal is focused", async () => {
    renderApp();
    await screen.findByTestId("activity-feed");

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

  it("should reset selection when navigating to a different view", async () => {
    useDashboardStore.setState({ selectedIndex: 2 });
    renderApp();
    await screen.findByTestId("activity-feed");

    act(() => fireKey("Escape"));
    expect(useDashboardStore.getState().selectedIndex).toBe(-1);
  });
});
