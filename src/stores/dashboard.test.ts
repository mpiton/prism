import { describe, it, expect, beforeEach } from "vitest";
import { useDashboardStore } from "./dashboard";

describe("useDashboardStore", () => {
  beforeEach(() => {
    useDashboardStore.setState({
      currentView: "overview",
      activeFilters: {},
      activeNavigableSection: null,
      navigableSectionRegistrations: [],
      selectedIndex: -1,
      navigableItems: [],
    });
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

  it("should remove a single filter via undefined", () => {
    useDashboardStore.getState().setFilter({ repo: "prism", priority: "high" });
    useDashboardStore.getState().setFilter({ repo: undefined });
    const filters = useDashboardStore.getState().activeFilters;
    expect(filters).toEqual({ priority: "high" });
    expect(Object.keys(filters)).not.toContain("repo");
  });

  it("should not add ghost keys from undefined values", () => {
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

  describe("keyboard navigation", () => {
    const items = [
      { url: "https://github.com/org/repo/pull/1" },
      { url: "https://github.com/org/repo/pull/2" },
      { url: "https://github.com/org/repo/pull/3" },
    ] as const;

    beforeEach(() => {
      useDashboardStore.setState({
        selectedIndex: -1,
        navigableItems: items,
      });
    });

    it("should have default selectedIndex of -1", () => {
      useDashboardStore.setState({ selectedIndex: -1, navigableItems: [] });
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    });

    it("should navigate down from -1 to 0", () => {
      useDashboardStore.getState().navigateList("down");
      expect(useDashboardStore.getState().selectedIndex).toBe(0);
    });

    it("should navigate up from -1 to 0", () => {
      useDashboardStore.getState().navigateList("up");
      expect(useDashboardStore.getState().selectedIndex).toBe(0);
    });

    it("should navigate down incrementally", () => {
      useDashboardStore.setState({ selectedIndex: 0 });
      useDashboardStore.getState().navigateList("down");
      expect(useDashboardStore.getState().selectedIndex).toBe(1);
    });

    it("should navigate up decrementally", () => {
      useDashboardStore.setState({ selectedIndex: 2 });
      useDashboardStore.getState().navigateList("up");
      expect(useDashboardStore.getState().selectedIndex).toBe(1);
    });

    it("should clamp at last item when navigating down", () => {
      useDashboardStore.setState({ selectedIndex: 2 });
      useDashboardStore.getState().navigateList("down");
      expect(useDashboardStore.getState().selectedIndex).toBe(2);
    });

    it("should clamp at first item when navigating up", () => {
      useDashboardStore.setState({ selectedIndex: 0 });
      useDashboardStore.getState().navigateList("up");
      expect(useDashboardStore.getState().selectedIndex).toBe(0);
    });

    it("should do nothing when list is empty", () => {
      useDashboardStore.setState({ navigableItems: [], selectedIndex: -1 });
      useDashboardStore.getState().navigateList("down");
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    });

    it("should reset selection when view changes", () => {
      useDashboardStore.setState({ selectedIndex: 2 });
      useDashboardStore.getState().setView("mine");
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
      expect(useDashboardStore.getState().navigableItems).toEqual([]);
    });

    it("should set navigable items", () => {
      const newItems = [{ url: "https://example.com" }];
      useDashboardStore.getState().setNavigableItems(newItems);
      expect(useDashboardStore.getState().navigableItems).toEqual(newItems);
    });

    it("should reset selectedIndex when navigable items change and index is out of bounds", () => {
      useDashboardStore.setState({ selectedIndex: 2 });
      useDashboardStore.getState().setNavigableItems([{ url: "https://example.com" }]);
      expect(useDashboardStore.getState().selectedIndex).toBe(0);
    });

    it("should keep selectedIndex when navigable items change but index is in bounds", () => {
      useDashboardStore.setState({ selectedIndex: 1 });
      useDashboardStore
        .getState()
        .setNavigableItems([
          { url: "https://a.com" },
          { url: "https://b.com" },
          { url: "https://c.com" },
        ]);
      expect(useDashboardStore.getState().selectedIndex).toBe(1);
    });

    it("should reset selectedIndex to -1 when items become empty", () => {
      useDashboardStore.setState({
        selectedIndex: 2,
        activeNavigableSection: "reviews",
        navigableSectionRegistrations: [{ sectionId: "reviews", items }],
      });
      useDashboardStore.getState().setNavigableItems([]);
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
      expect(useDashboardStore.getState().activeNavigableSection).toBeNull();
      expect(useDashboardStore.getState().navigableSectionRegistrations).toEqual([]);
    });

    it("should keep the first active section until ownership changes explicitly", () => {
      useDashboardStore
        .getState()
        .registerNavigableItems("reviews", [{ url: "https://example.com/reviews/1" }]);
      useDashboardStore
        .getState()
        .registerNavigableItems("issues", [{ url: "https://example.com/issues/1" }]);

      expect(useDashboardStore.getState().activeNavigableSection).toBe("reviews");
      expect(useDashboardStore.getState().navigableItems).toEqual([
        { url: "https://example.com/reviews/1" },
      ]);
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    });

    it("should switch to a later section when ownership changes explicitly", () => {
      useDashboardStore
        .getState()
        .registerNavigableItems("reviews", [{ url: "https://example.com/reviews/1" }]);
      useDashboardStore.setState({ activeNavigableSection: "issues" });
      useDashboardStore
        .getState()
        .registerNavigableItems("issues", [{ url: "https://example.com/issues/1" }]);

      expect(useDashboardStore.getState().activeNavigableSection).toBe("issues");
      expect(useDashboardStore.getState().navigableItems).toEqual([
        { url: "https://example.com/issues/1" },
      ]);
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    });

    it("should keep the active section when another section updates in the background", () => {
      useDashboardStore
        .getState()
        .registerNavigableItems("reviews", [{ url: "https://example.com/reviews/1" }]);
      useDashboardStore
        .getState()
        .registerNavigableItems("issues", [{ url: "https://example.com/issues/1" }]);
      useDashboardStore.setState({
        activeNavigableSection: "reviews",
        navigableItems: [{ url: "https://example.com/reviews/1" }],
        selectedIndex: 0,
      });

      useDashboardStore
        .getState()
        .registerNavigableItems("issues", [{ url: "https://example.com/issues/2" }]);

      expect(useDashboardStore.getState().activeNavigableSection).toBe("reviews");
      expect(useDashboardStore.getState().navigableItems).toEqual([
        { url: "https://example.com/reviews/1" },
      ]);
      expect(useDashboardStore.getState().selectedIndex).toBe(0);
    });

    it("should fall back to the previous registered section when the active one unregisters", () => {
      useDashboardStore
        .getState()
        .registerNavigableItems("reviews", [{ url: "https://example.com/reviews/1" }]);
      useDashboardStore.setState({
        activeNavigableSection: "issues",
        navigableItems: [{ url: "https://example.com/issues/1" }],
        selectedIndex: 0,
      });
      useDashboardStore
        .getState()
        .registerNavigableItems("issues", [{ url: "https://example.com/issues/1" }]);

      useDashboardStore.getState().unregisterNavigableSection("issues");

      expect(useDashboardStore.getState().activeNavigableSection).toBe("reviews");
      expect(useDashboardStore.getState().navigableItems).toEqual([
        { url: "https://example.com/reviews/1" },
      ]);
      expect(useDashboardStore.getState().selectedIndex).toBe(-1);
    });
  });
});
