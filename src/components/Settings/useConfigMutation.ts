import { useMutation, useQueryClient } from "@tanstack/react-query";
import { setConfig } from "../../lib/tauri";
import type { PartialAppConfig } from "../../lib/types";

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
      console.error("[Settings] config update failed:", err);
      onError("Failed to save setting. Please retry.");
      onResetDraft();
    },
  });
}
