import { type ReactElement } from "react";
import { render, screen, within, waitFor, act, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import type { AppConfig, AuthStatus, PersonalStats, Repo } from "../../lib/types";

vi.mock("../../lib/tauri", () => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
  listRepos: vi.fn(),
  setRepoEnabled: vi.fn(),
  authGetStatus: vi.fn(),
  authSetToken: vi.fn(),
  authLogout: vi.fn(),
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
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

function makeConfig(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    pollIntervalSecs: 300,
    maxActiveWorkspaces: 3,
    autoSuspendMinutes: 30,
    githubToken: "test-token",
    dataDir: null,
    workspacesDir: null,
    claudeAuthMode: "oauth",
    claudeAutoGenerateMd: false,
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
    totalWorkspaceCount: 1,
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
  afterEach(() => {
    vi.useRealTimers();
  });

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
    setupMocks(makeConfig(), [], makeAuthStatus({ connected: true, username: "alice" }));

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

    expect(await screen.findByRole("button", { name: /connect/i })).toBeInTheDocument();
    expect(screen.getByPlaceholderText("ghp_...")).toBeInTheDocument();
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
    const frontendRow = screen.getByText("frontend").closest("label");
    const backendRow = screen.getByText("backend").closest("label");

    expect(toggles).toHaveLength(2);
    expect(frontendRow).not.toBeNull();
    expect(backendRow).not.toBeNull();
    expect(within(frontendRow as HTMLElement).getByRole("checkbox")).toBeChecked();
    expect(within(backendRow as HTMLElement).getByRole("checkbox")).not.toBeChecked();
  });

  it("should call setRepoEnabled when repo toggle is clicked", async () => {
    const user = userEvent.setup();
    const repo = makeRepo(1, { name: "frontend", enabled: true });
    setupMocks(makeConfig(), [repo]);
    mockedSetRepoEnabled.mockResolvedValue({ ...repo, enabled: false });

    renderWithProviders(<Settings />);

    const reposSection = await screen.findByTestId("settings-repos");
    const toggle = within(reposSection).getByRole("checkbox");
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
        totalWorkspaceCount: 2,
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

  it("should have title attribute on repo name span", async () => {
    setupMocks(makeConfig(), [makeRepo(1, { name: "my-repo" })]);

    renderWithProviders(<Settings />);

    const repoSpan = await screen.findByText("my-repo");
    expect(repoSpan).toHaveAttribute("title", "my-repo");
  });

  it("should filter repos by search input", async () => {
    setupMocks(makeConfig(), [
      makeRepo(1, { name: "frontend", fullName: "org/frontend" }),
      makeRepo(2, { name: "backend", fullName: "org/backend" }),
      makeRepo(3, { name: "mobile", fullName: "org/mobile" }),
    ]);

    renderWithProviders(<Settings />);

    // Wait for repos to load, then activate fake timers for debounce
    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "front" } });
      vi.advanceTimersByTime(200);
    });

    expect(screen.getByText("frontend")).toBeInTheDocument();
    expect(screen.queryByText("backend")).not.toBeInTheDocument();
    expect(screen.queryByText("mobile")).not.toBeInTheDocument();
  });

  it("should wait for the debounced search before showing the match count label", async () => {
    setupMocks(makeConfig(), [
      makeRepo(1, { name: "frontend", fullName: "org/frontend" }),
      makeRepo(2, { name: "frontend-api", fullName: "org/frontend-api" }),
      makeRepo(3, { name: "backend", fullName: "org/backend" }),
    ]);

    renderWithProviders(<Settings />);

    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "front" } });
    });

    expect(screen.queryByText(/matching current filter/i)).not.toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(screen.getByText("2 matching current filter")).toBeInTheDocument();
  });

  it("should show enabled repositories count", async () => {
    setupMocks(makeConfig(), [
      makeRepo(1, { enabled: true }),
      makeRepo(2, { enabled: false }),
      makeRepo(3, { enabled: true }),
    ]);

    renderWithProviders(<Settings />);

    expect(await screen.findByText("2 of 3 repositories enabled")).toBeInTheDocument();
  });

  it("should group repositories by organization", async () => {
    setupMocks(makeConfig(), [
      makeRepo(1, { org: "zeta", name: "api", fullName: "zeta/api" }),
      makeRepo(2, { org: "alpha", name: "web", fullName: "alpha/web" }),
      makeRepo(3, { org: "alpha", name: "cli", fullName: "alpha/cli", enabled: false }),
    ]);

    renderWithProviders(<Settings />);

    expect(await screen.findByText("alpha/")).toBeInTheDocument();
    expect(screen.getByText("zeta/")).toBeInTheDocument();
    expect(screen.getByText("1/2 enabled")).toBeInTheDocument();
    expect(screen.getByText("1/1 enabled")).toBeInTheDocument();
  });

  it("should batch enable repositories matching the current filter", async () => {
    const repos = [
      makeRepo(1, { org: "org", name: "frontend", fullName: "org/frontend", enabled: false }),
      makeRepo(2, {
        org: "org",
        name: "frontend-docs",
        fullName: "org/frontend-docs",
        enabled: true,
      }),
      makeRepo(3, { org: "org", name: "backend", fullName: "org/backend", enabled: false }),
    ];
    setupMocks(makeConfig(), repos);
    mockedSetRepoEnabled.mockImplementation(async (repoId, enabled) => {
      const repo = repos.find((candidate) => candidate.id === repoId);
      return { ...(repo ?? makeRepo(99)), id: repoId, enabled };
    });

    renderWithProviders(<Settings />);

    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "front" } });
      vi.advanceTimersByTime(200);
    });
    vi.useRealTimers();

    fireEvent.click(screen.getByRole("button", { name: "Select all" }));

    await waitFor(() => expect(mockedSetRepoEnabled).toHaveBeenCalledTimes(1));
    expect(mockedSetRepoEnabled).toHaveBeenCalledWith("repo-1", true);
    expect(mockedSetRepoEnabled).not.toHaveBeenCalledWith("repo-3", true);
    expect(screen.getByText("2 matching current filter")).toBeInTheDocument();
  });

  it("should invert the current filtered selection", async () => {
    const repos = [
      makeRepo(1, { name: "frontend", fullName: "org/frontend", enabled: true }),
      makeRepo(2, { name: "frontend-api", fullName: "org/frontend-api", enabled: false }),
      makeRepo(3, { name: "backend", fullName: "org/backend", enabled: true }),
    ];
    setupMocks(makeConfig(), repos);
    mockedSetRepoEnabled.mockImplementation(async (repoId, enabled) => {
      const repo = repos.find((candidate) => candidate.id === repoId);
      return { ...(repo ?? makeRepo(99)), id: repoId, enabled };
    });

    renderWithProviders(<Settings />);

    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "front" } });
      vi.advanceTimersByTime(200);
    });
    vi.useRealTimers();

    fireEvent.click(screen.getByRole("button", { name: "Invert selection" }));

    await waitFor(() => expect(mockedSetRepoEnabled).toHaveBeenCalledTimes(2));
    expect(mockedSetRepoEnabled).toHaveBeenCalledWith("repo-1", false);
    expect(mockedSetRepoEnabled).toHaveBeenCalledWith("repo-2", true);
  });

  it("should refetch repos and show failed repo ids after a partial batch failure", async () => {
    const initialRepos = [
      makeRepo(1, { name: "frontend", fullName: "org/frontend", enabled: false }),
      makeRepo(2, { name: "frontend-api", fullName: "org/frontend-api", enabled: false }),
    ];
    const refetchedRepos = [
      makeRepo(1, { name: "frontend", fullName: "org/frontend", enabled: true }),
      makeRepo(2, { name: "frontend-api", fullName: "org/frontend-api", enabled: false }),
    ];

    mockedGetConfig.mockResolvedValue(makeConfig());
    mockedAuthGetStatus.mockResolvedValue(makeAuthStatus());
    mockedSetConfig.mockResolvedValue(makeConfig());
    mockedGetPersonalStats.mockResolvedValue(makePersonalStats());
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 50_000_000,
      dbSizeBytes: 1_048_576,
    });
    mockedListRepos.mockResolvedValueOnce(initialRepos).mockResolvedValueOnce(refetchedRepos);
    mockedSetRepoEnabled.mockImplementation(async (repoId, enabled) => {
      if (repoId === "repo-2") {
        throw new Error("toggle failed");
      }

      return (
        refetchedRepos.find((repo) => repo.id === repoId) ??
        makeRepo(99, { id: repoId, enabled, name: "fallback", fullName: `org/${repoId}` })
      );
    });

    renderWithProviders(<Settings />);

    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "front" } });
      vi.advanceTimersByTime(200);
    });
    vi.useRealTimers();

    fireEvent.click(screen.getByRole("button", { name: "Select all" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Failed to update repositories: repo-2",
    );
    await waitFor(() => expect(mockedListRepos).toHaveBeenCalledTimes(2));

    const frontendRow = screen.getByText("frontend").closest("label");
    const frontendApiRow = screen.getByText("frontend-api").closest("label");

    expect(frontendRow).not.toBeNull();
    expect(frontendApiRow).not.toBeNull();
    expect(within(frontendRow as HTMLElement).getByRole("checkbox")).toBeChecked();
    expect(within(frontendApiRow as HTMLElement).getByRole("checkbox")).not.toBeChecked();
  });

  it("should show no-match message when search returns empty", async () => {
    setupMocks(makeConfig(), [makeRepo(1, { name: "frontend", fullName: "org/frontend" })]);

    renderWithProviders(<Settings />);

    // Wait for repos to load, then activate fake timers for debounce
    await screen.findByText("frontend");
    vi.useFakeTimers();

    const input = screen.getByPlaceholderText("Filter repositories...");
    act(() => {
      fireEvent.change(input, { target: { value: "zzznomatch" } });
      vi.advanceTimersByTime(200);
    });

    expect(screen.getByText("No repos match")).toBeInTheDocument();
  });
});
