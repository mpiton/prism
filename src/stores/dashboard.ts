import { create } from "zustand";
import type { Priority, CiStatus } from "../lib/types/enums";

export type DashboardView =
  | "overview"
  | "reviews"
  | "mine"
  | "issues"
  | "feed"
  | "notifications"
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

export type NavigableSectionId = "reviews" | "mine" | "issues" | "notifications";

interface NavigableSectionRegistration {
  readonly sectionId: NavigableSectionId;
  readonly items: readonly NavigableItem[];
}

interface DashboardUiState {
  readonly currentView: DashboardView;
  readonly activeFilters: DashboardFilters;
  readonly selectedIndex: number;
  readonly navigableItems: readonly NavigableItem[];
  readonly activeNavigableSection: NavigableSectionId | null;
  readonly navigableSectionRegistrations: readonly NavigableSectionRegistration[];
  readonly focusMode: boolean;
}

interface DashboardActions {
  setView: (view: DashboardView) => void;
  setFilter: (filter: Partial<DashboardFilters>) => void;
  clearFilters: () => void;
  navigateList: (direction: "up" | "down") => void;
  setNavigableItems: (items: readonly NavigableItem[]) => void;
  registerNavigableItems: (sectionId: NavigableSectionId, items: readonly NavigableItem[]) => void;
  unregisterNavigableSection: (sectionId: NavigableSectionId) => void;
  toggleFocusMode: () => void;
}

type DashboardState = DashboardUiState & DashboardActions;

function pickActiveRegistration(
  registrations: readonly NavigableSectionRegistration[],
): NavigableSectionRegistration | null {
  for (let index = registrations.length - 1; index >= 0; index -= 1) {
    const registration = registrations[index];
    if (registration && registration.items.length > 0) return registration;
  }
  return null;
}

export const useDashboardStore = create<DashboardState>((set) => ({
  currentView: "overview",
  activeFilters: {},
  selectedIndex: -1,
  navigableItems: [],
  activeNavigableSection: null,
  navigableSectionRegistrations: [],
  focusMode: false,
  setView: (view) =>
    set({
      currentView: view,
      selectedIndex: -1,
      navigableItems: [],
      activeNavigableSection: null,
      navigableSectionRegistrations: [],
    }),
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
      if (items.length === 0) return { navigableItems: items, selectedIndex: -1 };
      const clamped = state.selectedIndex >= items.length ? items.length - 1 : state.selectedIndex;
      return {
        navigableItems: items,
        selectedIndex: clamped,
        activeNavigableSection: null,
        navigableSectionRegistrations: [],
      };
    }),
  registerNavigableItems: (sectionId, items) =>
    set((state) => {
      const registrations = [
        ...state.navigableSectionRegistrations.filter(
          (registration) => registration.sectionId !== sectionId,
        ),
      ];
      if (items.length > 0) registrations.push({ sectionId, items });

      const activeRegistration = pickActiveRegistration(registrations);
      if (!activeRegistration) {
        return {
          navigableSectionRegistrations: registrations,
          activeNavigableSection: null,
          navigableItems: [],
          selectedIndex: -1,
        };
      }

      const shouldKeepSelection = state.activeNavigableSection === activeRegistration.sectionId;
      const clampedSelectedIndex = shouldKeepSelection
        ? Math.min(state.selectedIndex, activeRegistration.items.length - 1)
        : -1;

      return {
        navigableSectionRegistrations: registrations,
        activeNavigableSection: activeRegistration.sectionId,
        navigableItems: activeRegistration.items,
        selectedIndex: clampedSelectedIndex,
      };
    }),
  unregisterNavigableSection: (sectionId) =>
    set((state) => {
      const registrations = state.navigableSectionRegistrations.filter(
        (registration) => registration.sectionId !== sectionId,
      );
      const activeRegistration = pickActiveRegistration(registrations);
      if (!activeRegistration) {
        return {
          navigableSectionRegistrations: [],
          activeNavigableSection: null,
          navigableItems: [],
          selectedIndex: -1,
        };
      }

      return {
        navigableSectionRegistrations: registrations,
        activeNavigableSection: activeRegistration.sectionId,
        navigableItems: activeRegistration.items,
        selectedIndex:
          state.activeNavigableSection === activeRegistration.sectionId ? state.selectedIndex : -1,
      };
    }),
  toggleFocusMode: () => set((state) => ({ focusMode: !state.focusMode })),
}));
