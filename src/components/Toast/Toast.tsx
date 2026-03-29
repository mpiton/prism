import { useCallback, useEffect, useRef, type ReactElement } from "react";
import { useNotifications } from "../../hooks/useNotifications";
import type { Notification } from "../../hooks/useNotifications";
import { useDashboardStore } from "../../stores/dashboard";
import type { DashboardView } from "../../stores/dashboard";

const AUTO_DISMISS_MS = 5_000;

const NOTIFICATION_META: Record<
  Notification["type"],
  { label: string; icon: string; view: DashboardView }
> = {
  review_request: { label: "Review Request", icon: "👀", view: "reviews" },
  ci_failure: { label: "CI Failure", icon: "❌", view: "mine" },
  pr_approved: { label: "PR Approved", icon: "✅", view: "mine" },
};

interface ToastItemProps {
  readonly notification: Notification;
  readonly onDismiss: (id: string) => void;
  readonly onNavigate: (view: DashboardView) => void;
}

function ToastItem({ notification, onDismiss, onNavigate }: ToastItemProps): ReactElement {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(null);
  const meta = NOTIFICATION_META[notification.type];

  useEffect(() => {
    timerRef.current = setTimeout(() => {
      onDismiss(notification.id);
    }, AUTO_DISMISS_MS);

    return () => {
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
      }
    };
  }, [notification.id, onDismiss]);

  const handleClick = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
    }
    onNavigate(meta.view);
    onDismiss(notification.id);
  }, [notification.id, onDismiss, onNavigate, meta.view]);

  return (
    <button
      type="button"
      onClick={handleClick}
      className="flex w-72 items-center gap-3 rounded-lg border border-border bg-bg-secondary p-3 shadow-lg transition-opacity hover:opacity-80"
    >
      <span className="text-lg" aria-hidden="true">{meta.icon}</span>
      <span className="text-sm font-medium text-fg">{meta.label}</span>
    </button>
  );
}

export function Toast(): ReactElement {
  const { notifications, clearNotification } = useNotifications();
  const setView = useDashboardStore((s) => s.setView);

  return (
    <div
      data-testid="toast-container"
      aria-live="polite"
      className="fixed bottom-4 right-4 z-50 flex flex-col gap-2"
    >
      {notifications.map((n) => (
        <ToastItem
          key={n.id}
          notification={n}
          onDismiss={clearNotification}
          onNavigate={setView}
        />
      ))}
    </div>
  );
}
