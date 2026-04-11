/**
 * Accessibility utilities for PRism.
 *
 * WCAG 2.4.7 — Focus Visible (Level AA).
 * Apply {@link FOCUS_RING} to every interactive element (button, link, input,
 * label acting as a control). It removes the browser default outline and
 * installs a high-contrast accent ring on keyboard focus only.
 *
 * The ring offset is `transparent` so the element's own background shows
 * through the 2px gap between the element edge and the accent ring. This is
 * resilient to any parent surface (`bg-bg`, `bg-surface`, `bg-surface-hover`,
 * …) and avoids the subtle dark-halo mismatch a hardcoded `ring-offset-bg`
 * would create on elements sitting on `bg-surface` panels.
 *
 * Keeping a single exported constant is the single source of truth for focus
 * styling across the app — do not duplicate the class list inline.
 */
export const FOCUS_RING =
  "outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-transparent";
