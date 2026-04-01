import { useEffect, useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listWorkspacesEnriched, onEvent } from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types";
import type { WorkspaceStatusInfo } from "../lib/types";

const STALE_TIME = 30_000;

export function useWorkspaceEnriched(enabled = true) {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["workspaces", "enriched"],
    queryFn: listWorkspacesEnriched,
    staleTime: STALE_TIME,
    enabled,
  });

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    onEvent(TAURI_EVENTS["workspace:state_changed"], async () => {
      await queryClient.invalidateQueries({ queryKey: ["workspaces", "enriched"] });
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisteners.push(fn);
        }
      })
      .catch((err: unknown) => {
        console.error("[useWorkspaceEnriched] failed to register listener:", err);
      });

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [queryClient, enabled]);

  const statusInfo: Record<string, WorkspaceStatusInfo> = useMemo(() => {
    const data = query.data ?? [];
    const map: Record<string, WorkspaceStatusInfo> = {};
    for (const entry of data) {
      map[entry.workspace.id] = {
        branch: entry.branch ?? "",
        ahead: entry.ahead,
        behind: entry.behind,
        ciStatus: entry.ciStatus,
        sessionName: entry.workspace.sessionId,
        sessionCount: entry.sessionCount,
        githubUrl: entry.githubUrl ?? "",
      };
    }
    return map;
  }, [query.data]);

  return { statusInfo, isLoading: query.isLoading };
}
