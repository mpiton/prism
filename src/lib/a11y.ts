/**
 * Accessibility utilities for PRism.
 *
 * WCAG 2.4.7 — Focus Visible (Level AA).
 * Apply {@link FOCUS_RING} to every interactive element (button, link, input,
 * label acting as a control). It removes the browser default outline and
 * installs a high-contrast accent ring on keyboard focus only, matching the
 * PRism dark theme (`bg` offset + `accent` ring color).
 *
 * Keeping a single exported constant is the single source of truth for focus
 * styling across the app — do not duplicate the class list inline.
 */
export const FOCUS_RING =
  "outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-bg";
