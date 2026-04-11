import { useQuery } from "@tanstack/react-query";
import { memo, type ReactElement, useCallback, useEffect, useMemo, useRef } from "react";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
import { useFilterableList } from "../../hooks/useFilterableList";
import { FOCUS_RING } from "../../lib/a11y";
import { listNotifications } from "../../lib/tauri";
import type { GithubNotification } from "../../lib/types/github";
import { FILTER_BUTTON_CLASS } from "../../lib/uiClasses";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { CardSkeleton, Skeleton } from "../atoms/Skeleton";
import { NotificationCard } from "./NotificationCard";

interface NotificationsProps {
  readonly onOpen: (url: string) => void;
}

type Tab = "unread" | "all";

const NOTIFICATION_TABS: Readonly<Record<Tab, (n: GithubNotification) => boolean>> = {
  unread: (n) => n.unread,
  all: () => true,
};

function NotificationsImpl({ onOpen }: NotificationsProps): ReactElement {
  const listRef = useRef<HTMLDivElement>(null);

  const notificationsQuery = useQuery<GithubNotification[]>({
    queryKey: ["github", "notifications"],
    queryFn: listNotifications,
    // 1 minute — notifications change frequently and the REST call is cheap.
    staleTime: 60_000,
  });

  const notifications = useMemo(() => notificationsQuery.data ?? [], [notificationsQuery.data]);

  const searchPredicate = useCallback(
    (n: GithubNotification, query: string): boolean =>
      [n.title, n.repo, n.reason].some((value) => value.toLowerCase().includes(query)),
    [],
  );

  const {
    tab,
    setTab,
    searchQuery,
    setSearchQuery,
    normalizedQuery,
    visibleItems: visible,
    tabCounts,
  } = useFilterableList<GithubNotification, Tab>({
    items: notifications,
    tabs: NOTIFICATION_TABS,
    defaultTab: "unread",
    searchPredicate,
  });

  useEffect(() => {
    listRef.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [tab, normalizedQuery]);

  const navItems = useMemo(() => visible.map((n) => ({ url: n.url })), [visible]);
  useRegisterNavigableItems(navItems);

  const isLoading = notificationsQuery.isLoading;
  const isFetching = notificationsQuery.isFetching;
  const isError = notificationsQuery.isError;
  const errorMessage =
    notificationsQuery.error instanceof Error
      ? notificationsQuery.error.message
      : "Failed to load notifications";

  return (
    <section
      data-testid="notifications"
      // aria-busy also reflects background refetches so screen readers hear
      // the activity when the query is silently re-fetched (e.g. after a
      // `github:updated` invalidation).
      aria-busy={isLoading || isFetching ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      {/* Header count shows the full, unfiltered notification total so the
          header number doesn't jitter as the user types in the search box. */}
      <SectionHead
        title="Notifications"
        count={isLoading ? undefined : notifications.length}
      />

      {isLoading ? (
        <>
          <div className="flex gap-1">
            <Skeleton className="h-11 w-20" />
            <Skeleton className="h-11 w-16" />
          </div>

          <div data-testid="notifications-loading" className="flex flex-col gap-1">
            {Array.from({ length: 3 }, (_, index) => (
              <CardSkeleton
                key={`notification-skeleton-${index}`}
                testId="notification-card-skeleton"
              />
            ))}
          </div>
        </>
      ) : isError ? (
        <div
          role="alert"
          className="rounded border border-red/40 bg-red/10 px-3 py-2 text-sm text-red"
        >
          {errorMessage}
        </div>
      ) : (
        <>
          <input
            type="search"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Filter notifications..."
            aria-label="Filter notifications"
            className={`${FOCUS_RING} w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted`}
          />

          <div className="flex gap-1" role="group" aria-label="Filter by state">
            <button
              type="button"
              aria-pressed={tab === "unread"}
              onClick={() => setTab("unread")}
              className={`${FILTER_BUTTON_CLASS} ${
                tab === "unread"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Unread {tabCounts.unread}
            </button>
            <button
              type="button"
              aria-pressed={tab === "all"}
              onClick={() => setTab("all")}
              className={`${FILTER_BUTTON_CLASS} ${
                tab === "all"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              All {tabCounts.all}
            </button>
          </div>

          {visible.length === 0 ? (
            <EmptyState
              icon="✓"
              message={tab === "unread" ? "No unread notifications" : "No notifications to display"}
            />
          ) : (
            <div ref={listRef} className="max-h-[600px] overflow-y-auto">
              <div className="flex flex-col gap-1">
                {visible.map((n) => (
                  <NotificationCard key={n.id} data={n} onOpen={onOpen} />
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}

export const Notifications = memo(NotificationsImpl);
