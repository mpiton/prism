import { useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  getGithubDashboard,
  getGithubStats,
  forceGithubSync,
  onEvent,
} from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types";

const STALE_TIME = 30_000;

function invalidateGitHub(queryClient: ReturnType<typeof useQueryClient>) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] }),
    queryClient.invalidateQueries({ queryKey: ["github", "stats"] }),
  ]);
}

export function useGitHubData(refetchInterval?: number) {
  const queryClient = useQueryClient();

  const dashboardQuery = useQuery({
    queryKey: ["github", "dashboard"],
    queryFn: getGithubDashboard,
    staleTime: STALE_TIME,
    refetchInterval,
  });

  const statsQuery = useQuery({
    queryKey: ["github", "stats"],
    queryFn: getGithubStats,
    staleTime: STALE_TIME,
    refetchInterval,
  });

  const syncMutation = useMutation({
    mutationFn: forceGithubSync,
    onSuccess: async () => {
      await invalidateGitHub(queryClient);
    },
  });

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    onEvent(TAURI_EVENTS["github:updated"], async () => {
      await invalidateGitHub(queryClient);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((err: unknown) => {
      console.error("[useGitHubData] failed to register github:updated listener:", err);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient]);

  return {
    dashboard: dashboardQuery.data ?? null,
    stats: statsQuery.data ?? null,
    isLoading: dashboardQuery.isLoading || statsQuery.isLoading,
    error: dashboardQuery.error ?? statsQuery.error ?? null,
    forceSync: syncMutation.mutate,
    isSyncing: syncMutation.isPending,
  };
}
