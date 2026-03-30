import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listWorkspaces, onEvent } from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types";
import type { Workspace, WorkspaceState } from "../lib/types";
import { useWorkspacesStore } from "../stores/workspaces";

const STALE_TIME = 30_000;

interface StateChangedPayload {
  readonly workspaceId: string;
  readonly newState: WorkspaceState;
}

interface ClaudeSessionPayload {
  readonly workspaceId: string;
  readonly sessionId: string;
}

interface UseWorkspaceResult {
  readonly workspaces: Workspace[] | null;
  readonly isLoading: boolean;
  readonly error: Error | null;
  readonly suspendedActiveWorkspace: string | null;
  readonly dismissSuspendedNotice: () => void;
}

function invalidateWorkspaceQueries(
  queryClient: ReturnType<typeof useQueryClient>,
) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
    queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] }),
  ]);
}

export function useWorkspace(): UseWorkspaceResult {
  const queryClient = useQueryClient();
  const activeWorkspaceId = useWorkspacesStore((s) => s.activeWorkspaceId);
  const [suspendedActiveWorkspace, setSuspendedActiveWorkspace] = useState<
    string | null
  >(null);

  // Keep a ref so the event handler always sees the latest value
  const activeIdRef = useRef(activeWorkspaceId);
  activeIdRef.current = activeWorkspaceId;

  const workspacesQuery = useQuery({
    queryKey: ["workspaces"],
    queryFn: listWorkspaces,
    staleTime: STALE_TIME,
  });

  // Clear stale suspended notice when the user switches active workspace
  useEffect(() => {
    setSuspendedActiveWorkspace((prev) =>
      prev !== null && prev !== activeWorkspaceId ? null : prev,
    );
  }, [activeWorkspaceId]);

  // Each effect invocation creates its own `cancelled` closure. On cleanup,
  // `cancelled` is set to `true` so that:
  // (a) any pending `onEvent` promise resolves and immediately unlistens, and
  // (b) async handlers skip state updates if the component has unmounted.
  useEffect(() => {
    let cancelled = false;
    let unlistenState: (() => void) | undefined;
    let unlistenClaude: (() => void) | undefined;

    onEvent<StateChangedPayload>(
      TAURI_EVENTS["workspace:state_changed"],
      async (payload) => {
        await invalidateWorkspaceQueries(queryClient);

        if (!cancelled && payload.workspaceId === activeIdRef.current) {
          if (payload.newState === "suspended") {
            setSuspendedActiveWorkspace(payload.workspaceId);
          } else if (payload.newState === "active") {
            setSuspendedActiveWorkspace(null);
          }
        }
      },
    )
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenState = fn;
        }
      })
      .catch((err: unknown) => {
        console.error(
          "[useWorkspace] failed to register workspace:state_changed listener:",
          err,
        );
      });

    onEvent<ClaudeSessionPayload>(
      TAURI_EVENTS["workspace:claude_session"],
      async () => {
        await invalidateWorkspaceQueries(queryClient);
      },
    )
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlistenClaude = fn;
        }
      })
      .catch((err: unknown) => {
        console.error(
          "[useWorkspace] failed to register workspace:claude_session listener:",
          err,
        );
      });

    return () => {
      cancelled = true;
      unlistenState?.();
      unlistenClaude?.();
    };
  }, [queryClient]);

  const dismissSuspendedNotice = useCallback(() => {
    setSuspendedActiveWorkspace(null);
  }, []);

  return {
    workspaces: workspacesQuery.data ?? null,
    isLoading: workspacesQuery.isLoading,
    error: workspacesQuery.error ?? null,
    suspendedActiveWorkspace,
    dismissSuspendedNotice,
  };
}
