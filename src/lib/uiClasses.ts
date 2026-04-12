import { FOCUS_RING } from "./a11y";

/**
 * Shared class strings for interactive controls that appear in more than one
 * feature surface. Centralising them mirrors the single-source-of-truth
 * treatment given to {@link FOCUS_RING}: a shape tweak only needs to happen
 * once.
 */

/**
 * Square-ish filter button used on list headers (Issues, MyPRs, ReviewQueue,
 * ActivityFeed). Sized to the 44px minimum touch target.
 */
export const FILTER_BUTTON_CLASS = `${FOCUS_RING} inline-flex min-h-11 min-w-11 items-center justify-center rounded px-3 text-xs leading-none transition-colors`;

/**
 * Inline action button used alongside filters (e.g. "Mark all read" in the
 * activity feed).
 */
export const ACTION_BUTTON_CLASS = `${FOCUS_RING} inline-flex min-h-11 items-center rounded px-3 text-xs transition-colors`;

/**
 * Inline control (e.g. native `<select>`) used alongside filter buttons in list
 * headers (ReviewQueue, MyPRs, Issues).
 */
export const INLINE_CONTROL_CLASS = `${FOCUS_RING} min-h-11 rounded px-3 text-xs transition-colors`;

/**
 * Shared visual treatment for the currently keyboard-selected list item.
 * Matches the accent focus language without moving DOM focus.
 */
export const SELECTED_ITEM_CLASS =
  "border-accent bg-surface-hover ring-2 ring-accent ring-offset-2 ring-offset-transparent";
