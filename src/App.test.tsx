import { describe, expect, it, beforeEach } from "vitest";
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
