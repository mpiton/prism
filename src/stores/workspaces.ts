import { create } from "zustand";

interface WorkspacesUiState {
  readonly activeWorkspaceId: string | null;
}

interface WorkspacesActions {
  setActiveWorkspace: (id: string) => void;
  clearActiveWorkspace: () => void;
}

type WorkspacesState = WorkspacesUiState & WorkspacesActions;

export const useWorkspacesStore = create<WorkspacesState>((set) => ({
  activeWorkspaceId: null,
  setActiveWorkspace: (id) => set({ activeWorkspaceId: id }),
  clearActiveWorkspace: () => set({ activeWorkspaceId: null }),
}));
