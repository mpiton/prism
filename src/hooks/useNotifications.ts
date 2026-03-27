import { useCallback, useEffect, useRef, useState } from "react";
import { onEvent } from "../lib/tauri";
import { TAURI_EVENTS } from "../lib/types";

type NotificationType = "review_request" | "ci_failure" | "pr_approved";

export interface Notification {
  id: string;
  type: NotificationType;
  payload: unknown;
  timestamp: number;
}

let nextId = 0;

function makeId(): string {
  nextId += 1;
  return `notif-${nextId}`;
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
  const cancelledRef = useRef(false);

  useEffect(() => {
    cancelledRef.current = false;
    const unlistenFns: Array<() => void> = [];

    for (const { event, type } of EVENT_TYPE_MAP) {
      onEvent(event, (payload: unknown) => {
        const notification: Notification = {
          id: makeId(),
          type,
          payload,
          timestamp: Date.now(),
        };
        setNotifications((prev) => [...prev, notification]);
      })
        .then((unlisten) => {
          if (cancelledRef.current) {
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
      cancelledRef.current = true;
      for (const fn of unlistenFns) {
        fn();
      }
    };
  }, []);

  const clearNotification = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }, []);

  return { notifications, clearNotification };
}
