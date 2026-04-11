import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useFilterableList } from "./useFilterableList";

interface TestItem {
  readonly id: number;
  readonly title: string;
  readonly state: "open" | "closed";
}

const ITEMS: readonly TestItem[] = [
  { id: 1, title: "Alpha bug", state: "open" },
  { id: 2, title: "Beta bug", state: "closed" },
  { id: 3, title: "Gamma feature", state: "open" },
  { id: 4, title: "Delta feature", state: "closed" },
];

const tabs = {
  open: (item: TestItem) => item.state === "open",
  closed: (item: TestItem) => item.state === "closed",
} as const;

const searchPredicate = (item: TestItem, query: string): boolean =>
  item.title.toLowerCase().includes(query);

function renderFilterableList(items: readonly TestItem[] = ITEMS) {
  return renderHook(() =>
    useFilterableList<TestItem, "open" | "closed">({
      items,
      tabs,
      defaultTab: "open",
      searchPredicate,
    }),
  );
}

describe("useFilterableList", () => {
  it("starts on the default tab with empty search", () => {
    const { result } = renderFilterableList();

    expect(result.current.tab).toBe("open");
    expect(result.current.searchQuery).toBe("");
    expect(result.current.normalizedQuery).toBe("");
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([1, 3]);
  });

  it("exposes tab counts computed from the full items set when no search is active", () => {
    const { result } = renderFilterableList();

    expect(result.current.tabCounts).toEqual({ open: 2, closed: 2 });
  });

  it("switches tabs when setTab is called", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setTab("closed");
    });

    expect(result.current.tab).toBe("closed");
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([2, 4]);
  });

  it("filters items through the search predicate after setSearchQuery", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("feature");
    });

    expect(result.current.searchQuery).toBe("feature");
    expect(result.current.normalizedQuery).toBe("feature");
    // On the "open" tab, only "Gamma feature" matches.
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([3]);
  });

  it("normalizes the search query by trimming and lowercasing before passing to the predicate", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("  ALPHA  ");
    });

    expect(result.current.normalizedQuery).toBe("alpha");
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([1]);
  });

  it("treats a whitespace-only query as empty and keeps all items visible", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("   ");
    });

    expect(result.current.normalizedQuery).toBe("");
    // All open items remain (search short-circuits).
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([1, 3]);
  });

  it("short-circuits and does not invoke the search predicate when the query is empty", () => {
    let calls = 0;
    const countingPredicate = (item: TestItem, query: string): boolean => {
      calls += 1;
      return item.title.toLowerCase().includes(query);
    };

    const { result } = renderHook(() =>
      useFilterableList<TestItem, "open" | "closed">({
        items: ITEMS,
        tabs,
        defaultTab: "open",
        searchPredicate: countingPredicate,
      }),
    );

    // Initial render with an empty query should not call the predicate.
    expect(calls).toBe(0);
    expect(result.current.visibleItems).toHaveLength(2);
  });

  it("combines search and tab filtering", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("bug");
    });
    act(() => {
      result.current.setTab("closed");
    });

    expect(result.current.tab).toBe("closed");
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([2]);
  });

  it("computes tab counts from the search-filtered set, not the raw items", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("feature");
    });

    expect(result.current.tabCounts).toEqual({ open: 1, closed: 1 });
  });

  it("preserves searchQuery when the tab changes", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("bug");
    });
    act(() => {
      result.current.setTab("closed");
    });

    expect(result.current.searchQuery).toBe("bug");
  });

  it("preserves tab when searchQuery changes", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setTab("closed");
    });
    act(() => {
      result.current.setSearchQuery("delta");
    });

    expect(result.current.tab).toBe("closed");
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([4]);
  });

  it("updates visibleItems when the items prop changes", () => {
    const initialItems: readonly TestItem[] = [{ id: 1, title: "only", state: "open" }];
    const { result, rerender } = renderHook(
      (props: { items: readonly TestItem[] }) =>
        useFilterableList<TestItem, "open" | "closed">({
          items: props.items,
          tabs,
          defaultTab: "open",
          searchPredicate,
        }),
      { initialProps: { items: initialItems } },
    );

    expect(result.current.visibleItems).toHaveLength(1);

    rerender({ items: ITEMS });

    expect(result.current.visibleItems.map((item) => item.id)).toEqual([1, 3]);
  });

  it("returns an empty visibleItems array when the search matches nothing", () => {
    const { result } = renderFilterableList();

    act(() => {
      result.current.setSearchQuery("nothing-matches-this");
    });

    expect(result.current.visibleItems).toEqual([]);
    expect(result.current.filteredItems).toEqual([]);
    expect(result.current.tabCounts).toEqual({ open: 0, closed: 0 });
  });

  it("exposes filteredItems (post-search, pre-tab) for total counts", () => {
    const { result } = renderFilterableList();

    // No search: filteredItems equals all items.
    expect(result.current.filteredItems).toHaveLength(4);

    act(() => {
      result.current.setSearchQuery("bug");
    });

    // After searching "bug": two items match regardless of tab.
    expect(result.current.filteredItems.map((item) => item.id)).toEqual([1, 2]);
    // The active tab is still "open", so visibleItems drops "Beta bug" (closed).
    expect(result.current.visibleItems.map((item) => item.id)).toEqual([1]);
  });
});
