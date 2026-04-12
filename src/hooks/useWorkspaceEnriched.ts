import { useEffect, useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listWorkspacesEnriched, onEvent } from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types/tauri";
import type { WorkspaceListEntry, WorkspaceStatusInfo } from "../lib/types/workspace";

// 5 minutes: workspace freshness is driven by the workspace:state_changed
// Tauri event, so time-based polling can be relaxed.
const STALE_TIME = 300_000;

export function useWorkspaceEnriched() {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["workspaces", "enriched"],
    queryFn: listWorkspacesEnriched,
    staleTime: STALE_TIME,
  });

  useEffect(() => {
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
  }, [queryClient]);

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

  const entries: readonly WorkspaceListEntry[] = useMemo(() => query.data ?? [], [query.data]);

  return { statusInfo, entries, isLoading: query.isLoading };
}
