import { type ReactElement } from "react";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi, beforeEach } from "vitest";
import type { AppConfig, AuthStatus, PersonalStats, Repo } from "../../lib/types";

vi.mock("../../lib/tauri", () => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
  listRepos: vi.fn(),
  setRepoEnabled: vi.fn(),
  authGetStatus: vi.fn(),
  getPersonalStats: vi.fn(),
  getMemoryUsage: vi.fn(),
}));

import {
  getConfig,
  setConfig,
  listRepos,
  setRepoEnabled,
  authGetStatus,
  getPersonalStats,
  getMemoryUsage,
} from "../../lib/tauri";

const mockedGetConfig = vi.mocked(getConfig);
const mockedSetConfig = vi.mocked(setConfig);
const mockedListRepos = vi.mocked(listRepos);
const mockedSetRepoEnabled = vi.mocked(setRepoEnabled);
const mockedAuthGetStatus = vi.mocked(authGetStatus);
const mockedGetPersonalStats = vi.mocked(getPersonalStats);
const mockedGetMemoryUsage = vi.mocked(getMemoryUsage);

function renderWithProviders(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

function makeConfig(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    pollIntervalSecs: 300,
    maxActiveWorkspaces: 3,
    autoSuspendMinutes: 30,
    githubToken: "test-token",
    dataDir: null,
    workspacesDir: null,
    ...overrides,
  };
}

function makeAuthStatus(overrides: Partial<AuthStatus> = {}): AuthStatus {
  return {
    connected: true,
    username: "matvei",
    error: null,
    ...overrides,
  };
}

function makeRepo(n: number, overrides: Partial<Repo> = {}): Repo {
  return {
    id: `repo-${n}`,
    org: "org",
    name: `repo-${n}`,
    fullName: `org/repo-${n}`,
    url: `https://github.com/org/repo-${n}`,
    defaultBranch: "main",
    isArchived: false,
    enabled: true,
    localPath: null,
    lastSyncAt: null,
    ...overrides,
  };
}

function makePersonalStats(overrides: Partial<PersonalStats> = {}): PersonalStats {
  return {
    prsMergedThisWeek: 3,
    avgReviewResponseHours: 2.5,
    reviewsGivenThisWeek: 7,
    activeWorkspaceCount: 1,
    ...overrides,
  };
}

function setupMocks(
  config: AppConfig = makeConfig(),
  repos: Repo[] = [makeRepo(1), makeRepo(2)],
  auth: AuthStatus = makeAuthStatus(),
  stats: PersonalStats = makePersonalStats(),
) {
  mockedGetConfig.mockResolvedValue(config);
  mockedListRepos.mockResolvedValue(repos);
  mockedAuthGetStatus.mockResolvedValue(auth);
  mockedSetConfig.mockResolvedValue(config);
  mockedGetPersonalStats.mockResolvedValue(stats);
  mockedGetMemoryUsage.mockResolvedValue({
    rssBytes: 50_000_000,
    dbSizeBytes: 1_048_576,
  });
}

// Lazy import so mocks are set up before module loads
let Settings: () => ReactElement;

beforeEach(async () => {
  vi.clearAllMocks();
  const mod = await import("./Settings");
  Settings = mod.Settings;
});

describe("Settings", () => {
  it("should render all config sections", async () => {
    setupMocks();

    renderWithProviders(<Settings />);

    expect(await screen.findByTestId("settings-github")).toBeInTheDocument();
    expect(screen.getByTestId("settings-workspaces")).toBeInTheDocument();
    expect(screen.getByTestId("settings-repos")).toBeInTheDocument();
  });

  it("should show loading state while config is fetching", () => {
    mockedGetConfig.mockReturnValue(new Promise(() => {}));
    mockedListRepos.mockReturnValue(new Promise(() => {}));
    mockedAuthGetStatus.mockReturnValue(new Promise(() => {}));

    renderWithProviders(<Settings />);

    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it("should display poll interval value from config", async () => {
    setupMocks(makeConfig({ pollIntervalSecs: 600 }));

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/poll interval/i);
    expect(input).toHaveValue(600);
  });

  it("should display max active workspaces from config", async () => {
    setupMocks(makeConfig({ maxActiveWorkspaces: 5 }));

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/max active/i);
    expect(input).toHaveValue(5);
  });

  it("should display auth status when connected", async () => {
    setupMocks(
      makeConfig(),
      [],
      makeAuthStatus({ connected: true, username: "alice" }),
    );

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/alice/)).toBeInTheDocument();
    expect(screen.getByText(/connected/i)).toBeInTheDocument();
  });

  it("should display disconnected auth status", async () => {
    setupMocks(
      makeConfig({ githubToken: null }),
      [],
      makeAuthStatus({ connected: false, username: null }),
    );

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/not connected/i)).toBeInTheDocument();
  });

  it("should show checking state while auth is loading", async () => {
    mockedGetConfig.mockResolvedValue(makeConfig());
    mockedListRepos.mockResolvedValue([]);
    mockedAuthGetStatus.mockReturnValue(new Promise(() => {}));

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/checking/i)).toBeInTheDocument();
  });

  it("should call config_set on poll interval change", async () => {
    const user = userEvent.setup();
    const updatedConfig = makeConfig({ pollIntervalSecs: 120 });
    setupMocks();
    mockedSetConfig.mockResolvedValue(updatedConfig);

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/poll interval/i);
    await user.clear(input);
    await user.type(input, "120");
    await user.tab();

    expect(mockedSetConfig).toHaveBeenCalledWith({ pollIntervalSecs: 120 });
  });

  it("should reject values below minimum", async () => {
    const user = userEvent.setup();
    setupMocks();

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/poll interval/i);
    await user.clear(input);
    await user.type(input, "0");
    await user.tab();

    expect(mockedSetConfig).not.toHaveBeenCalled();
    expect(input).toHaveValue(300);
  });

  it("should call config_set on max workspaces change", async () => {
    const user = userEvent.setup();
    const updatedConfig = makeConfig({ maxActiveWorkspaces: 5 });
    setupMocks();
    mockedSetConfig.mockResolvedValue(updatedConfig);

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/max active/i);
    await user.clear(input);
    await user.type(input, "5");
    await user.tab();

    expect(mockedSetConfig).toHaveBeenCalledWith({ maxActiveWorkspaces: 5 });
  });

  it("should show repos with toggle", async () => {
    setupMocks(makeConfig(), [
      makeRepo(1, { name: "frontend", enabled: true }),
      makeRepo(2, { name: "backend", enabled: false }),
    ]);

    renderWithProviders(<Settings />);

    expect(await screen.findByText("frontend")).toBeInTheDocument();
    expect(screen.getByText("backend")).toBeInTheDocument();

    const reposSection = screen.getByTestId("settings-repos");
    const toggles = within(reposSection).getAllByRole("checkbox");
    expect(toggles).toHaveLength(2);
    expect(toggles[0]).toBeChecked();
    expect(toggles[1]).not.toBeChecked();
  });

  it("should call setRepoEnabled when repo toggle is clicked", async () => {
    const user = userEvent.setup();
    const repo = makeRepo(1, { name: "frontend", enabled: true });
    setupMocks(makeConfig(), [repo]);
    mockedSetRepoEnabled.mockResolvedValue({ ...repo, enabled: false });

    renderWithProviders(<Settings />);

    const toggle = await screen.findByRole("checkbox");
    await user.click(toggle);

    expect(mockedSetRepoEnabled).toHaveBeenCalledWith("repo-1", false);
  });

  it("should show error state when config fetch fails", async () => {
    mockedGetConfig.mockRejectedValue(new Error("DB error"));
    mockedListRepos.mockResolvedValue([]);
    mockedAuthGetStatus.mockResolvedValue(makeAuthStatus());

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/failed to load/i)).toBeInTheDocument();
  });

  it("should show loading state for repos section while fetching", async () => {
    mockedGetConfig.mockResolvedValue(makeConfig());
    mockedListRepos.mockReturnValue(new Promise(() => {}));
    mockedAuthGetStatus.mockResolvedValue(makeAuthStatus());

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/loading repositories/i)).toBeInTheDocument();
  });

  it("should show error when repos fetch fails", async () => {
    mockedGetConfig.mockResolvedValue(makeConfig());
    mockedListRepos.mockRejectedValue(new Error("Network error"));
    mockedAuthGetStatus.mockResolvedValue(makeAuthStatus());

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/failed to load repositories/i)).toBeInTheDocument();
  });

  it("should show save error when config mutation fails", async () => {
    const user = userEvent.setup();
    setupMocks();
    mockedSetConfig.mockRejectedValue(new Error("DB locked"));

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/poll interval/i);
    await user.clear(input);
    await user.type(input, "120");
    await user.tab();

    expect(await screen.findByRole("alert")).toHaveTextContent(/failed to save/i);
  });

  it("should reset draft to original value when config mutation fails", async () => {
    const user = userEvent.setup();
    setupMocks(makeConfig({ pollIntervalSecs: 300 }));
    mockedSetConfig.mockRejectedValue(new Error("DB locked"));

    renderWithProviders(<Settings />);

    const input = await screen.findByLabelText(/poll interval/i);
    await user.clear(input);
    await user.type(input, "120");
    await user.tab();

    await screen.findByRole("alert");
    await waitFor(() => expect(input).toHaveValue(300));
  });

  it("should render stats section", async () => {
    setupMocks(
      makeConfig(),
      [],
      makeAuthStatus(),
      makePersonalStats({
        prsMergedThisWeek: 5,
        avgReviewResponseHours: 3.2,
        reviewsGivenThisWeek: 12,
        activeWorkspaceCount: 2,
      }),
    );

    renderWithProviders(<Settings />);

    // Wait for stats data to load (the section renders immediately in loading state)
    expect(await screen.findByText("3.2h")).toBeInTheDocument();
    const statsSection = screen.getByTestId("settings-stats");
    expect(within(statsSection).getByText(/^5$/)).toBeInTheDocument();
    expect(within(statsSection).getByText(/^12$/)).toBeInTheDocument();
    expect(within(statsSection).getByText(/^2$/)).toBeInTheDocument();
  });

  it("should show stats unavailable when stats fetch fails", async () => {
    setupMocks();
    mockedGetPersonalStats.mockRejectedValue(new Error("DB error"));

    renderWithProviders(<Settings />);

    expect(await screen.findByText(/stats unavailable/i)).toBeInTheDocument();
  });

  it("should render debug section with memory info", async () => {
    setupMocks();

    renderWithProviders(<Settings />);

    const debugSection = await screen.findByTestId("settings-debug");
    expect(debugSection).toBeInTheDocument();
    expect(within(debugSection).getByText(/debug/i)).toBeInTheDocument();
  });

  it("should show N/A for non-finite avg review response hours", async () => {
    setupMocks(
      makeConfig(),
      [],
      makeAuthStatus(),
      makePersonalStats({ avgReviewResponseHours: Infinity }),
    );

    renderWithProviders(<Settings />);

    expect(await screen.findByText("N/A")).toBeInTheDocument();
  });
});
