# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2026-04-13

Second public release of PRism, focused on triage depth, workspace reliability, and product hardening after `0.1.0`.

### Highlights

- Added a dedicated GitHub Notifications view with sidebar badge support and richer notification cards.
- Expanded list views with repo and label filters, inline search, focus mode, and clearer overview drill-down behavior.
- Improved command palette coverage with grouped actions, navigation shortcuts, update links, and better keyboard accessibility.
- Strengthened PR Workspaces with safer PTY isolation, archived and suspended workspace handling, auto-clone flows, terminal resume, and orphaned session reconciliation.
- Extended settings and observability with keyboard shortcuts, repository controls, personal stats, debug info, memory monitoring, and structured logging.

### Hardening

- Hardened the Tauri security posture with a strict CSP, safer external link opening, and dependency updates for known advisories.
- Improved accessibility across views with stronger contrast, visible focus states, dialog titles, touch target sizing, and screen-reader affordances.
- Reduced UI overhead with memoized views, lucide icon centralization, font subsetting, virtualized issues lists, and higher GitHub query stale times.

### Quality

- Added broad frontend and backend test coverage for notifications, filters, navigation, workspaces, and GitHub integration paths.
- Kept Linux release packaging through GitHub Actions for AppImage and `.deb` bundles.

## [0.1.0] - 2026-04-10

First public release of PRism.
