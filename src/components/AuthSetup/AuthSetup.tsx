import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { authSetToken, authGetStatus, authLogout } from "../../lib/tauri";

function extractErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

export function AuthSetup() {
  const [token, setToken] = useState("");
  const queryClient = useQueryClient();

  const statusQuery = useQuery({
    queryKey: ["auth", "status"],
    queryFn: authGetStatus,
    staleTime: 60_000,
  });

  const setTokenMutation = useMutation({
    mutationFn: authSetToken,
    onSuccess: () => {
      setToken("");
      queryClient.invalidateQueries({ queryKey: ["auth", "status"] });
    },
  });

  const logoutMutation = useMutation({
    mutationFn: authLogout,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["auth", "status"] });
    },
  });

  const status = statusQuery.data;
  const isConnected = status?.connected === true;
  const transientError = status?.error ?? null;
  const mutationError = setTokenMutation.error ? extractErrorMessage(setTokenMutation.error) : null;
  const logoutError = logoutMutation.error ? extractErrorMessage(logoutMutation.error) : null;
  const displayError = mutationError ?? logoutError ?? transientError;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = token.trim();
    if (trimmed) {
      setTokenMutation.mutate(trimmed);
    }
  }

  if (statusQuery.isLoading) {
    return (
      <div className="flex items-center justify-center p-6">
        <p className="text-sm text-muted">Checking authentication…</p>
      </div>
    );
  }

  if (statusQuery.isError) {
    return (
      <div className="flex flex-col items-center gap-4 rounded-lg border border-border bg-surface p-6">
        <p role="alert" className="text-sm text-red">
          {extractErrorMessage(statusQuery.error)}
        </p>
        <button
          type="button"
          onClick={() => statusQuery.refetch()}
          className="rounded-md border border-border bg-bg px-4 py-2 text-sm text-fg transition-colors hover:bg-surface"
        >
          Retry
        </button>
      </div>
    );
  }

  if (isConnected) {
    return (
      <div className="flex flex-col items-center gap-4 rounded-lg border border-border bg-surface p-6">
        <p className="text-sm text-muted">Connected as</p>
        <p className="text-lg font-semibold text-accent">{status.username ?? "unknown"}</p>
        <button
          type="button"
          onClick={() => logoutMutation.mutate()}
          disabled={logoutMutation.isPending}
          className="rounded-md border border-border bg-bg px-4 py-2 text-sm text-fg transition-colors hover:bg-surface disabled:opacity-50"
        >
          Disconnect
        </button>
        {logoutError && (
          <p role="alert" className="text-sm text-red">
            {logoutError}
          </p>
        )}
      </div>
    );
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-col gap-4 rounded-lg border border-border bg-surface p-6"
    >
      <label htmlFor="auth-token" className="text-sm font-medium text-fg">
        GitHub Token
      </label>
      <input
        id="auth-token"
        type="password"
        value={token}
        onChange={(e) => setToken(e.target.value)}
        placeholder="ghp_..."
        autoComplete="off"
        className="rounded-md border border-border bg-bg px-3 py-2 font-mono text-sm text-fg placeholder:text-muted focus:border-accent focus:outline-none"
      />

      {displayError && (
        <p role="alert" className="text-sm text-red">
          {displayError}
        </p>
      )}

      <button
        type="submit"
        disabled={!token.trim() || setTokenMutation.isPending}
        className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-bg transition-colors hover:opacity-90 disabled:opacity-50"
      >
        Connect
      </button>
    </form>
  );
}
