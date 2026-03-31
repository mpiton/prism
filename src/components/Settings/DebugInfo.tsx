import type { ReactElement } from "react";
import { useQuery } from "@tanstack/react-query";
import { getMemoryUsage } from "../../lib/tauri";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${Math.round(kb * 10) / 10} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${Math.round(mb * 10) / 10} MB`;
  const gb = mb / 1024;
  return `${Math.round(gb * 10) / 10} GB`;
}

export function DebugInfo(): ReactElement {
  const memoryQuery = useQuery({
    queryKey: ["debug", "memory"],
    queryFn: getMemoryUsage,
    staleTime: 10_000,
    refetchInterval: 30_000,
  });

  return (
    <div data-testid="settings-debug" className="flex flex-col gap-3">
      <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Debug</h2>
      {memoryQuery.isLoading ? (
        <span className="text-dim text-sm">Loading memory info...</span>
      ) : memoryQuery.data ? (
        <>
          <div className="flex items-center justify-between text-sm">
            <span className="text-dim">Process RSS</span>
            <span className="font-mono text-white">
              {memoryQuery.data.rssBytes > 0 ? formatBytes(memoryQuery.data.rssBytes) : "N/A"}
            </span>
          </div>
          <div className="flex items-center justify-between text-sm">
            <span className="text-dim">Database size</span>
            <span className="font-mono text-white">{formatBytes(memoryQuery.data.dbSizeBytes)}</span>
          </div>
        </>
      ) : memoryQuery.error ? (
        <span className="text-dim text-sm">Memory info unavailable</span>
      ) : null}
    </div>
  );
}
