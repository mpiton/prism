import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore } from "./settings";
import type { AppConfig } from "../lib/types";

const fakeConfig: AppConfig = {
  pollIntervalSecs: 60,
  maxActiveWorkspaces: 3,
  githubToken: "FAKE_TOKEN_FOR_TESTING",
  dataDir: "/home/user/.local/share/prism",
  workspacesDir: "/home/user/.prism/workspaces",
};

describe("useSettingsStore", () => {
  beforeEach(() => {
    useSettingsStore.setState({ config: null, isLoading: false });
  });

  it("should default to null", () => {
    const state = useSettingsStore.getState();
    expect(state.config).toBeNull();
    expect(state.isLoading).toBe(false);
  });

  it("should set config", () => {
    useSettingsStore.getState().setConfig(fakeConfig);
    expect(useSettingsStore.getState().config).toEqual(fakeConfig);
  });

  it("should accept a config with null optional fields", () => {
    const minimal: AppConfig = {
      pollIntervalSecs: 30,
      maxActiveWorkspaces: 1,
      githubToken: null,
      dataDir: null,
      workspacesDir: null,
    };
    useSettingsStore.getState().setConfig(minimal);
    expect(useSettingsStore.getState().config).toEqual(minimal);
  });

  it("should replace config entirely on second setConfig call", () => {
    useSettingsStore.getState().setConfig(fakeConfig);
    const updated: AppConfig = { ...fakeConfig, pollIntervalSecs: 120 };
    useSettingsStore.getState().setConfig(updated);
    expect(useSettingsStore.getState().config).toEqual(updated);
  });

  it("should allow setting config back to null", () => {
    useSettingsStore.getState().setConfig(fakeConfig);
    useSettingsStore.getState().setConfig(null);
    expect(useSettingsStore.getState().config).toBeNull();
  });

  it("should set loading state", () => {
    useSettingsStore.getState().setLoading(true);
    expect(useSettingsStore.getState().isLoading).toBe(true);

    useSettingsStore.getState().setLoading(false);
    expect(useSettingsStore.getState().isLoading).toBe(false);
  });

  it("should not mutate previous state on setConfig", () => {
    const before = useSettingsStore.getState();
    useSettingsStore.getState().setConfig(fakeConfig);
    const after = useSettingsStore.getState();
    expect(before).not.toBe(after);
    expect(before.config).toBeNull();
    expect(after.config).toEqual(fakeConfig);
  });
});
