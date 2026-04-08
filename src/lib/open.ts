import { openUrl as tauriOpen } from "@tauri-apps/plugin-opener";

export function openUrl(url: string): void {
  tauriOpen(url).catch((err: unknown) => {
    console.warn("[openUrl] failed to open", url, err);
  });
}
