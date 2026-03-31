import { type ReactElement } from "react";
import { render, screen, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi, beforeEach } from "vitest";

vi.mock("../../lib/tauri", () => ({
  getMemoryUsage: vi.fn(),
}));

import { getMemoryUsage } from "../../lib/tauri";

const mockedGetMemoryUsage = vi.mocked(getMemoryUsage);

function renderWithProviders(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

let DebugInfo: () => ReactElement;

beforeEach(async () => {
  vi.clearAllMocks();
  const mod = await import("./DebugInfo");
  DebugInfo = mod.DebugInfo;
});

describe("DebugInfo", () => {
  it("should render debug section header", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 50_000_000,
      dbSizeBytes: 1_234_567,
    });

    renderWithProviders(<DebugInfo />);

    const section = await screen.findByTestId("settings-debug");
    expect(within(section).getByText(/debug/i)).toBeInTheDocument();
  });

  it("should display RSS in human-readable MB format", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 52_428_800, // 50 MB
      dbSizeBytes: 1_048_576,
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("50 MB")).toBeInTheDocument();
  });

  it("should display database size in human-readable format", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 50_000_000,
      dbSizeBytes: 1_048_576, // 1 MB
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("1 MB")).toBeInTheDocument();
  });

  it("should show loading state while fetching", () => {
    mockedGetMemoryUsage.mockReturnValue(new Promise(() => {}));

    renderWithProviders(<DebugInfo />);

    expect(screen.getByText(/loading memory/i)).toBeInTheDocument();
  });

  it("should show unavailable when fetch fails", async () => {
    mockedGetMemoryUsage.mockRejectedValue(new Error("failed"));

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText(/memory info unavailable/i)).toBeInTheDocument();
  });

  it("should format small sizes in KB", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 50_000_000,
      dbSizeBytes: 512_000, // ~500 KB
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("500 KB")).toBeInTheDocument();
  });

  it("should format byte-level sizes", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 50_000_000,
      dbSizeBytes: 512,
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("512 B")).toBeInTheDocument();
  });

  it("should format GB-level sizes", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 1_610_612_736, // 1.5 GB
      dbSizeBytes: 1_048_576,
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("1.5 GB")).toBeInTheDocument();
  });

  it("should show N/A when RSS is 0 (non-Linux)", async () => {
    mockedGetMemoryUsage.mockResolvedValue({
      rssBytes: 0,
      dbSizeBytes: 1_048_576,
    });

    renderWithProviders(<DebugInfo />);

    expect(await screen.findByText("N/A")).toBeInTheDocument();
  });
});
