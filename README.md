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

- [ ] Real-time dashboard: review queue, your PRs, issues, activity feed
- [ ] Priority scoring for PRs (size, age, CI status, review urgency)
- [ ] Multi-repo support with per-repo configuration
- [ ] Native system notifications for review requests, CI failures, approvals
- [ ] Command palette and keyboard-driven workflow
- [ ] PR Workspaces with embedded terminal and Claude Code sessions
- [ ] Offline-first with SQLite cache

## Tech stack

| Layer | Technologies |
|-------|-------------|
| **Backend** | Rust, Tauri 2.10, tokio, sqlx (SQLite), octocrab, graphql_client |
| **Frontend** | React 19, TypeScript, Zustand, TanStack Query v5, Tailwind CSS 4 |
| **Tooling** | oxlint, oxfmt, Vitest, cargo test |

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
