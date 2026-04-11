import { useCallback, useEffect, useState } from "react";
import { onEvent } from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types/tauri";

type NotificationType = "review_request" | "ci_failure" | "pr_approved";

export interface Notification {
  id: string;
  type: NotificationType;
  payload: unknown;
  timestamp: number;
}

const EVENT_TYPE_MAP: ReadonlyArray<{
  event: (typeof TAURI_EVENTS)[keyof typeof TAURI_EVENTS];
  type: NotificationType;
}> = [
  { event: TAURI_EVENTS["notification:review_request"], type: "review_request" },
  { event: TAURI_EVENTS["notification:ci_failure"], type: "ci_failure" },
  { event: TAURI_EVENTS["notification:pr_approved"], type: "pr_approved" },
];

export function useNotifications() {
  const [notifications, setNotifications] = useState<Notification[]>([]);

  useEffect(() => {
    let cancelled = false;
    const unlistenFns: Array<() => void> = [];

    for (const { event, type } of EVENT_TYPE_MAP) {
      onEvent(event, (payload: unknown) => {
        if (cancelled) {
          return;
        }
        const notification: Notification = {
          id: crypto.randomUUID(),
          type,
          payload,
          timestamp: Date.now(),
        };
        setNotifications((prev) => [...prev, notification]);
      })
        .then((unlisten) => {
          if (cancelled) {
            unlisten();
          } else {
            unlistenFns.push(unlisten);
          }
        })
        .catch((err: unknown) => {
          console.error(`[useNotifications] failed to register ${event} listener:`, err);
        });
    }

    return () => {
      cancelled = true;
      for (const fn of unlistenFns) {
        try {
          fn();
        } catch (err) {
          console.error("[useNotifications] unlisten failed:", err);
        }
      }
    };
  }, []);

  const clearNotification = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }, []);

  return { notifications, clearNotification };
}
