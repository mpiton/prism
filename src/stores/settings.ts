import { create } from "zustand";
import type { AppConfig } from "../lib/types";

interface SettingsUiState {
  readonly config: AppConfig | null;
  readonly isLoading: boolean;
}

interface SettingsActions {
  setConfig: (config: AppConfig | null) => void;
  setLoading: (loading: boolean) => void;
}

type SettingsState = SettingsUiState & SettingsActions;

export const useSettingsStore = create<SettingsState>((set) => ({
  config: null,
  isLoading: false,
  setConfig: (config) => set({ config }),
  setLoading: (loading) => set({ isLoading: loading }),
}));
