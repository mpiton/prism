import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { ptyWrite, ptyResize, onEvent } from "../../lib/tauri";
import type { PtyOutput } from "../../lib/types";

const PRISM_THEME = {
  background: "#0a0a09",
  foreground: "#d4d0c8",
  cursor: "#c8ff00",
  cursorAccent: "#0a0a09",
  selectionBackground: "#3a3a3a",
  black: "#0a0a09",
  red: "#e06c75",
  green: "#98c379",
  yellow: "#e5c07b",
  blue: "#61afef",
  magenta: "#c678dd",
  cyan: "#56b6c2",
  white: "#d4d0c8",
} as const;

interface TerminalProps {
  readonly ptyId: string;
  readonly disabled?: boolean;
}

export function Terminal({ ptyId, disabled }: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
      theme: PRISM_THEME,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);
    term.open(container);
    fitAddon.fit();

    // stdin: forward user keystrokes to PTY
    term.onData((data) => {
      if (disabledRef.current) return;
      ptyWrite({ workspaceId: ptyId, data }).catch((err: unknown) => {
        console.error("[Terminal] ptyWrite failed:", err);
      });
    });

    // stdout: listen for PTY output events
    const unlistenPromise = onEvent<PtyOutput>("workspace:stdout", (payload) => {
      if (payload.workspaceId === ptyId) {
        term.write(payload.data);
      }
    });

    // resize: observe container and notify PTY
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      if (disabledRef.current) return;
      const dims = fitAddon.proposeDimensions();
      if (dims) {
        ptyResize({ workspaceId: ptyId, cols: dims.cols, rows: dims.rows }).catch(
          (err: unknown) => {
            console.error("[Terminal] ptyResize failed:", err);
          },
        );
      }
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      unlistenPromise
        .then((unlisten) => unlisten())
        .catch(() => {});
      term.dispose();
    };
  }, [ptyId]);

  return (
    <div className="relative h-full w-full">
      <div
        ref={containerRef}
        data-testid={`terminal-${ptyId}`}
        className="h-full w-full overflow-hidden bg-[#0a0a09]"
      />
      {disabled && (
        <div
          data-testid="terminal-suspended-overlay"
          className="absolute inset-0 flex items-center justify-center bg-black/70"
        >
          <p className="text-sm text-neutral-400">
            Workspace suspended
          </p>
        </div>
      )}
    </div>
  );
}
