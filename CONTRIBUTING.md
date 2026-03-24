# Contributing to PRism

Thanks for your interest in contributing to PRism!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/<your-username>/prism.git`
3. Install dependencies: `npm install`
4. Create a branch: `git checkout -b feat/my-feature`
5. Make your changes
6. Run checks (see below)
7. Commit and push
8. Open a Pull Request

## Development Setup

Ensure you have the [prerequisites](README.md#prerequisites) installed, then:

```bash
npm install
cargo tauri dev
```

## Code Quality

Before submitting a PR, run all checks:

```bash
# Rust
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# TypeScript
npx oxlint .
npx vitest run
```

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

**Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

**Scopes**: `acl`, `github`, `cache`, `workspace`, `ui`, `settings`, `notifications`

Examples:

- `feat(workspace): add PTY resize support`
- `fix(github): handle rate limit 403 responses`
- `test(cache): add CRUD tests for repos table`

## Pull Request Guidelines

- Keep PRs focused on a single change
- Include tests for new functionality
- Update documentation if needed
- Fill out the PR template completely
- Ensure CI passes before requesting review

## Reporting Issues

Use the [issue templates](https://github.com/mpiton/prism/issues/new/choose) to report bugs or request features.
