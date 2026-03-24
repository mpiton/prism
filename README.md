# PRism

[![CI](https://github.com/mpiton/prism/actions/workflows/ci.yml/badge.svg)](https://github.com/mpiton/prism/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

GitHub Review Dashboard & PR Workspaces — a Tauri desktop app for centralized code review.

PRism aggregates your GitHub review requests, pull requests, issues, and activity into a single real-time dashboard with priority scoring. PR Workspaces provide isolated development environments (git worktrees + embedded terminal + persistent Claude Code sessions) for instant context switching.

<!-- TODO: Add screenshot -->

## Features

- [ ] Real-time dashboard: review queue, your PRs, issues, activity feed
- [ ] Priority scoring for PRs (size, age, CI status, review urgency)
- [ ] Multi-repo support with per-repo configuration
- [ ] Native system notifications for review requests, CI failures, approvals
- [ ] Command palette and keyboard-driven workflow
- [ ] PR Workspaces with embedded terminal and Claude Code sessions
- [ ] Offline-first with SQLite cache

## Tech Stack

- **Backend**: Rust, Tauri 2.10, tokio, sqlx (SQLite), octocrab, graphql_client
- **Frontend**: React 19, TypeScript, Zustand, TanStack Query v5, Tailwind CSS 4
- **Tooling**: oxlint, Vitest, cargo test

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) >= 20 + npm
- Tauri system dependencies: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Development

```bash
npm install
cargo tauri dev
```

## Build

```bash
cargo tauri build
```

## Testing

```bash
# Rust
cargo test
cargo clippy -- -D warnings

# TypeScript
npx vitest run
npx vitest run --coverage
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT — see [LICENSE](LICENSE)
