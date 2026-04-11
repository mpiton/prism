import { useMutation, useQueryClient } from "@tanstack/react-query";
import { setConfig } from "../../lib/tauri";
import type { PartialAppConfig } from "../../lib/types/config";

interface UseConfigMutationOptions {
  readonly onError: (message: string | null) => void;
  readonly onResetDraft: () => void;
}

export function useConfigMutation({ onError, onResetDraft }: UseConfigMutationOptions) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (partial: PartialAppConfig) => setConfig(partial),
    onMutate: () => onError(null),
    onSuccess: (updated) => {
      queryClient.setQueryData(["config"], updated);
    },
    onError: (err: unknown) => {
      console.error("[useConfigMutation] config update failed:", err);
      onError("Failed to save setting. Please retry.");
      onResetDraft();
    },
    // Safety net for out-of-order concurrent saves: if two sections mutate at
    // once, the slower response could overwrite the cache with an older
    // snapshot via `setQueryData`. Invalidating on settle pulls the canonical
    // state from the backend as the source of truth.
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["config"] });
    },
  });
}
