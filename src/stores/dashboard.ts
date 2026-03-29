import { create } from "zustand";
import type { Priority, CiStatus } from "../lib/types";

export type DashboardView =
  | "overview"
  | "reviews"
  | "mine"
  | "issues"
  | "feed"
  | "workspaces"
  | "settings";

export interface DashboardFilters {
  readonly repo?: string;
  readonly priority?: Priority;
  readonly ciStatus?: CiStatus;
}

export interface NavigableItem {
  readonly url: string;
  readonly workspaceId?: string;
}

interface DashboardUiState {
  readonly currentView: DashboardView;
  readonly activeFilters: DashboardFilters;
  readonly selectedIndex: number;
  readonly navigableItems: readonly NavigableItem[];
}

interface DashboardActions {
  setView: (view: DashboardView) => void;
  setFilter: (filter: Partial<DashboardFilters>) => void;
  clearFilters: () => void;
  navigateList: (direction: "up" | "down") => void;
  setNavigableItems: (items: readonly NavigableItem[]) => void;
}

type DashboardState = DashboardUiState & DashboardActions;

export const useDashboardStore = create<DashboardState>((set) => ({
  currentView: "overview",
  activeFilters: {},
  selectedIndex: -1,
  navigableItems: [],
  setView: (view) =>
    set({ currentView: view, selectedIndex: -1, navigableItems: [] }),
  setFilter: (filter) =>
    set((state) => {
      const prev = state.activeFilters;
      const next: DashboardFilters = {
        repo: "repo" in filter ? filter.repo : prev.repo,
        priority: "priority" in filter ? filter.priority : prev.priority,
        ciStatus: "ciStatus" in filter ? filter.ciStatus : prev.ciStatus,
      };
      const cleaned = Object.fromEntries(
        Object.entries(next).filter(([, v]) => v !== undefined),
      ) as DashboardFilters;
      return { activeFilters: cleaned };
    }),
  clearFilters: () => set({ activeFilters: {} }),
  navigateList: (direction) =>
    set((state) => {
      const len = state.navigableItems.length;
      if (len === 0) return state;
      if (state.selectedIndex < 0) return { selectedIndex: 0 };
      const next =
        direction === "down"
          ? Math.min(state.selectedIndex + 1, len - 1)
          : Math.max(state.selectedIndex - 1, 0);
      return { selectedIndex: next };
    }),
  setNavigableItems: (items) =>
    set((state) => {
      const clamped =
        state.selectedIndex >= items.length && items.length > 0
          ? items.length - 1
          : state.selectedIndex < 0 && items.length > 0
            ? state.selectedIndex
            : state.selectedIndex;
      return { navigableItems: items, selectedIndex: clamped };
    }),
}));
