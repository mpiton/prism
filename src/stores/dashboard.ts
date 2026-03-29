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

interface DashboardUiState {
  readonly currentView: DashboardView;
  readonly activeFilters: DashboardFilters;
}

interface DashboardActions {
  setView: (view: DashboardView) => void;
  setFilter: (filter: Partial<DashboardFilters>) => void;
  clearFilters: () => void;
}

type DashboardState = DashboardUiState & DashboardActions;

export const useDashboardStore = create<DashboardState>((set) => ({
  currentView: "overview",
  activeFilters: {},
  setView: (view) => set({ currentView: view }),
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
}));
