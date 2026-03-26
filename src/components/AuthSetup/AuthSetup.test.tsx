import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthSetup } from "./AuthSetup";

// Mock the tauri IPC wrappers
vi.mock("../../lib/tauri", () => ({
  authSetToken: vi.fn(),
  authGetStatus: vi.fn(),
  authLogout: vi.fn(),
}));

import { authSetToken, authGetStatus, authLogout } from "../../lib/tauri";

const mockedAuthSetToken = vi.mocked(authSetToken);
const mockedAuthGetStatus = vi.mocked(authGetStatus);
const mockedAuthLogout = vi.mocked(authLogout);

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

describe("AuthSetup", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("should show token input when disconnected", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: null,
    });

    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText(/token/i)).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
  });

  it("should show username when connected", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: true,
      username: "octocat",
      error: null,
    });

    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText("octocat")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /disconnect/i })).toBeInTheDocument();
    expect(screen.queryByLabelText(/token/i)).not.toBeInTheDocument();
  });

  it("should display error on invalid token", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: null,
    });
    mockedAuthSetToken.mockRejectedValue("invalid or expired token");

    const user = userEvent.setup();
    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText(/token/i)).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/token/i), "ghp_invalid123");
    await user.click(screen.getByRole("button", { name: /connect/i }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/invalid or expired token/i);
    });
  });

  it("should call auth_set_token on submit with trimmed token", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: null,
    });
    mockedAuthSetToken.mockResolvedValue("octocat");

    const user = userEvent.setup();
    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText(/token/i)).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/token/i), "  ghp_validtoken123  ");
    await user.click(screen.getByRole("button", { name: /connect/i }));

    await waitFor(() => {
      expect(mockedAuthSetToken).toHaveBeenCalled();
      // eslint-disable-next-line @typescript-eslint/no-non-null-assertion -- guarded by toHaveBeenCalled above
      expect(mockedAuthSetToken.mock.calls[0]![0]).toBe("ghp_validtoken123");
    });
  });

  it("should call auth_logout on disconnect", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: true,
      username: "octocat",
      error: null,
    });
    mockedAuthLogout.mockResolvedValue(undefined);

    const user = userEvent.setup();
    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /disconnect/i })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /disconnect/i }));

    await waitFor(() => {
      expect(mockedAuthLogout).toHaveBeenCalledOnce();
    });
  });

  it("should disable submit button when token is empty", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: null,
    });

    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /connect/i })).toBeDisabled();
    });
  });

  it("should show loading state during submission", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: null,
    });
    // Never resolves — keeps loading state
    mockedAuthSetToken.mockReturnValue(new Promise(() => {}));

    const user = userEvent.setup();
    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText(/token/i)).toBeInTheDocument();
    });

    await user.type(screen.getByLabelText(/token/i), "ghp_token123");
    await user.click(screen.getByRole("button", { name: /connect/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /connect/i })).toBeDisabled();
    });
  });

  it("should display transient error from status", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: false,
      username: null,
      error: "GitHub API error: request failed: timeout",
    });

    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/timeout/i);
    });
  });

  it("should show error state when status query fails", async () => {
    mockedAuthGetStatus.mockRejectedValue("IPC connection failed");

    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/IPC connection failed/i);
    });
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
  });

  it("should display logout error when disconnect fails", async () => {
    mockedAuthGetStatus.mockResolvedValue({
      connected: true,
      username: "octocat",
      error: null,
    });
    mockedAuthLogout.mockRejectedValue("keyring access denied");

    const user = userEvent.setup();
    render(<AuthSetup />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /disconnect/i })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /disconnect/i }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(/keyring access denied/i);
    });
  });
});
