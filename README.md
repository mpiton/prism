<p align="center">
  <img src="src-tauri/icons/prism-logo.svg" width="128" height="128" alt="PRism logo">
</p>

<h1 align="center">PRism</h1>

<p align="center">
  GitHub Review Dashboard & PR Workspaces
</p>

<p align="center">
  <a href="https://github.com/mpiton/prism/actions/workflows/ci.yml"><img src="https://github.com/mpiton/prism/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Tauri-2.10-blue?logo=tauri" alt="Tauri 2.10">
  <img src="https://img.shields.io/badge/React-19-blue?logo=react" alt="React 19">
  <img src="https://img.shields.io/badge/Rust-stable-orange?logo=rust" alt="Rust">
</p>

---

A Tauri desktop app that aggregates your GitHub review requests, pull requests, issues, and activity into a single real-time dashboard with priority scoring. PR Workspaces provide isolated development environments (git worktrees + embedded terminal + persistent Claude Code sessions) for instant context switching.

## Features

- Unified dashboard for review requests, pull requests, issues, activity, notifications, and overview metrics
- Priority scoring, repo and label filters, focus mode, inline search, and keyboard-first navigation
- Multi-repo support with per-repo enablement and local repository path management
- Native system notifications, tray integration, and command palette shortcuts
- PR Workspaces with git worktrees, embedded terminal, suspend/resume lifecycle, and Claude Code session persistence
- Offline-first SQLite cache with background GitHub sync

## Install

Prebuilt Linux packages are published on the [GitHub Releases](https://github.com/mpiton/prism/releases) page.

- `.AppImage` for a portable install
- `.deb` for Debian/Ubuntu-based systems

## Tech stack

| Layer        | Technologies                                                     |
| ------------ | ---------------------------------------------------------------- |
| **Backend**  | Rust, Tauri 2.10, tokio, sqlx (SQLite), reqwest, graphql_client  |
| **Frontend** | React 19, TypeScript, Zustand, TanStack Query v5, Tailwind CSS 4 |
| **Tooling**  | oxlint, oxfmt, Vitest, cargo test                                |

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) >= 20 + npm
- Tauri system dependencies: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

## Testing

```bash
npm run check
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT — see [LICENSE](LICENSE)
