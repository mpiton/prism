import { describe, it, expect, beforeEach } from "vitest";
import { useDashboardStore } from "./dashboard";

describe("useDashboardStore", () => {
  beforeEach(() => {
    useDashboardStore.setState({ currentView: "overview", activeFilters: {} });
  });

  it("should have default view as overview", () => {
    const state = useDashboardStore.getState();
    expect(state.currentView).toBe("overview");
  });

  it("should have empty filters by default", () => {
    const state = useDashboardStore.getState();
    expect(state.activeFilters).toEqual({});
  });

  it("should set view", () => {
    useDashboardStore.getState().setView("reviews");
    expect(useDashboardStore.getState().currentView).toBe("reviews");

    useDashboardStore.getState().setView("settings");
    expect(useDashboardStore.getState().currentView).toBe("settings");
  });

  it("should set filter", () => {
    useDashboardStore.getState().setFilter({ repo: "prism" });
    expect(useDashboardStore.getState().activeFilters).toEqual({
      repo: "prism",
    });
  });

  it("should merge filters when setting multiple", () => {
    useDashboardStore.getState().setFilter({ repo: "prism" });
    useDashboardStore.getState().setFilter({ priority: "high" });
    expect(useDashboardStore.getState().activeFilters).toEqual({
      repo: "prism",
      priority: "high",
    });
  });

  it("should overwrite existing filter key", () => {
    useDashboardStore.getState().setFilter({ repo: "prism" });
    useDashboardStore.getState().setFilter({ repo: "other-repo" });
    expect(useDashboardStore.getState().activeFilters.repo).toBe("other-repo");
  });

  it("should clear filters", () => {
    useDashboardStore.getState().setFilter({ repo: "prism", priority: "high" });
    useDashboardStore.getState().clearFilters();
    expect(useDashboardStore.getState().activeFilters).toEqual({});
  });

  it("should handle all filter types together", () => {
    useDashboardStore.getState().setFilter({
      repo: "prism",
      priority: "high",
      ciStatus: "success",
    });
    expect(useDashboardStore.getState().activeFilters).toEqual({
      repo: "prism",
      priority: "high",
      ciStatus: "success",
    });
  });

  it("should ignore undefined values in setFilter", () => {
    useDashboardStore.getState().setFilter({ repo: "prism" });
    useDashboardStore.getState().setFilter({ priority: undefined });
    const filters = useDashboardStore.getState().activeFilters;
    expect(filters).toEqual({ repo: "prism" });
    expect(Object.keys(filters)).toEqual(["repo"]);
  });

  it("should not mutate previous state on setView", () => {
    const before = useDashboardStore.getState();
    useDashboardStore.getState().setView("workspaces");
    const after = useDashboardStore.getState();
    expect(before).not.toBe(after);
    expect(before.currentView).toBe("overview");
    expect(after.currentView).toBe("workspaces");
  });

  it("should not mutate previous filters on setFilter", () => {
    useDashboardStore.getState().setFilter({ repo: "prism" });
    const filtersBefore = useDashboardStore.getState().activeFilters;
    useDashboardStore.getState().setFilter({ priority: "critical" });
    const filtersAfter = useDashboardStore.getState().activeFilters;
    expect(filtersBefore).not.toBe(filtersAfter);
    expect(filtersBefore).toEqual({ repo: "prism" });
  });
});
