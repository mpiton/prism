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
      const sanitized = Object.fromEntries(
        Object.entries(filter).filter(([, v]) => v !== undefined),
      ) as Partial<DashboardFilters>;
      return { activeFilters: { ...state.activeFilters, ...sanitized } };
    }),
  clearFilters: () => set({ activeFilters: {} }),
}));
