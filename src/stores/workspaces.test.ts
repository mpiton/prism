import { describe, it, expect, beforeEach } from "vitest";
import { useWorkspacesStore } from "./workspaces";

describe("useWorkspacesStore", () => {
  beforeEach(() => {
    useWorkspacesStore.setState({ activeWorkspaceId: null });
  });

  it("should default to null", () => {
    const state = useWorkspacesStore.getState();
    expect(state.activeWorkspaceId).toBeNull();
  });

  it("should set active workspace", () => {
    useWorkspacesStore.getState().setActiveWorkspace("ws-123");
    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-123");
  });

  it("should clear active workspace", () => {
    useWorkspacesStore.getState().setActiveWorkspace("ws-123");
    useWorkspacesStore.getState().clearActiveWorkspace();
    expect(useWorkspacesStore.getState().activeWorkspaceId).toBeNull();
  });

  it("should replace active workspace on second call", () => {
    useWorkspacesStore.getState().setActiveWorkspace("ws-1");
    useWorkspacesStore.getState().setActiveWorkspace("ws-2");
    expect(useWorkspacesStore.getState().activeWorkspaceId).toBe("ws-2");
  });

  it("should not mutate previous state on setActiveWorkspace", () => {
    const before = useWorkspacesStore.getState();
    useWorkspacesStore.getState().setActiveWorkspace("ws-abc");
    const after = useWorkspacesStore.getState();
    expect(before).not.toBe(after);
    expect(before.activeWorkspaceId).toBeNull();
    expect(after.activeWorkspaceId).toBe("ws-abc");
  });
});
