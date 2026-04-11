import { useMemo, useState } from "react";

export interface FilterableListConfig<T, TabKey extends string> {
  readonly items: readonly T[];
  readonly tabs: Readonly<Record<TabKey, (item: T) => boolean>>;
  /**
   * The tab selected on mount. Only read during the first render —
   * subsequent changes are ignored. Use `setTab` to change the tab after mount.
   */
  readonly defaultTab: TabKey;
  /**
   * Called once per item with the already-normalized (trimmed + lowercased)
   * search query. Must be stable between renders (wrap in `useCallback`) for
   * the hook's internal memoization to be effective.
   */
  readonly searchPredicate: (item: T, normalizedQuery: string) => boolean;
}

export interface FilterableListResult<T, TabKey extends string> {
  readonly tab: TabKey;
  readonly setTab: (tab: TabKey) => void;
  readonly searchQuery: string;
  readonly setSearchQuery: (query: string) => void;
  readonly normalizedQuery: string;
  /** Items that match the search query, before any tab filter is applied. */
  readonly filteredItems: readonly T[];
  /** Items that match the search query AND the active tab predicate. */
  readonly visibleItems: readonly T[];
  readonly tabCounts: Readonly<Record<TabKey, number>>;
}

/**
 * Encapsulates the shared "filter by tab + search" pattern used by list views
 * such as Issues and MyPRs.
 *
 * The hook owns the tab and search-query state, normalizes the query once
 * (trim + lowercase), short-circuits the search predicate for empty queries,
 * and exposes both the visible items for the active tab and per-tab counts
 * computed from the search-filtered set.
 *
 * For best performance, pass a stable `tabs` object (module-level constant)
 * and a `useCallback`-wrapped `searchPredicate`.
 */
export function useFilterableList<T, TabKey extends string>({
  items,
  tabs,
  defaultTab,
  searchPredicate,
}: FilterableListConfig<T, TabKey>): FilterableListResult<T, TabKey> {
  const [tab, setTab] = useState<TabKey>(defaultTab);
  const [searchQuery, setSearchQuery] = useState("");

  const normalizedQuery = useMemo(() => searchQuery.trim().toLowerCase(), [searchQuery]);

  const filteredItems = useMemo<readonly T[]>(
    () =>
      normalizedQuery.length === 0
        ? items
        : items.filter((item) => searchPredicate(item, normalizedQuery)),
    [items, normalizedQuery, searchPredicate],
  );

  const tabCounts = useMemo<Readonly<Record<TabKey, number>>>(() => {
    const tabKeys = Object.keys(tabs) as TabKey[];
    return Object.fromEntries(
      tabKeys.map((key) => [key, filteredItems.filter(tabs[key]).length]),
    ) as Record<TabKey, number>;
  }, [filteredItems, tabs]);

  const visibleItems = useMemo<readonly T[]>(
    () => filteredItems.filter(tabs[tab]),
    [filteredItems, tabs, tab],
  );

  return {
    tab,
    setTab,
    searchQuery,
    setSearchQuery,
    normalizedQuery,
    filteredItems,
    visibleItems,
    tabCounts,
  };
}
