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
}

export function Terminal({ ptyId }: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null);

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
    <div
      ref={containerRef}
      data-testid={`terminal-${ptyId}`}
      className="h-full w-full overflow-hidden bg-[#0a0a09]"
    />
  );
}
