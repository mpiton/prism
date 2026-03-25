-- PRism initial schema: 10 tables, indexes, and default config values.
-- Applied by sqlx::migrate!() at application startup.

-- ── repos ──────────────────────────────────────────────────────────

CREATE TABLE repos (
    id            TEXT PRIMARY KEY,
    org           TEXT NOT NULL,
    name          TEXT NOT NULL,
    full_name     TEXT NOT NULL,
    url           TEXT NOT NULL,
    default_branch TEXT NOT NULL,
    is_archived   INTEGER NOT NULL DEFAULT 0,
    enabled       INTEGER NOT NULL DEFAULT 1,
    local_path    TEXT,
    last_sync_at  TEXT,
    UNIQUE(org, name)
);

-- ── pull_requests ──────────────────────────────────────────────────

CREATE TABLE pull_requests (
    id          TEXT PRIMARY KEY,
    number      INTEGER NOT NULL,
    title       TEXT NOT NULL,
    author      TEXT NOT NULL,
    state       TEXT NOT NULL,
    ci_status   TEXT NOT NULL,
    priority    TEXT NOT NULL,
    repo_id     TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    labels      TEXT NOT NULL DEFAULT '[]',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(repo_id, number)
);

CREATE INDEX idx_pull_requests_repo_id ON pull_requests(repo_id);
CREATE INDEX idx_pull_requests_state   ON pull_requests(state);

-- ── review_requests ────────────────────────────────────────────────

CREATE TABLE review_requests (
    id               TEXT PRIMARY KEY,
    pull_request_id  TEXT NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
    reviewer         TEXT NOT NULL,
    status           TEXT NOT NULL,
    requested_at     TEXT NOT NULL
);

CREATE INDEX idx_review_requests_pull_request_id ON review_requests(pull_request_id);
CREATE INDEX idx_review_requests_reviewer        ON review_requests(reviewer);

-- ── reviews ────────────────────────────────────────────────────────

CREATE TABLE reviews (
    id               TEXT PRIMARY KEY,
    pull_request_id  TEXT NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
    reviewer         TEXT NOT NULL,
    status           TEXT NOT NULL,
    body             TEXT,
    submitted_at     TEXT NOT NULL
);

CREATE INDEX idx_reviews_pull_request_id ON reviews(pull_request_id);

-- ── issues ─────────────────────────────────────────────────────────

CREATE TABLE issues (
    id          TEXT PRIMARY KEY,
    number      INTEGER NOT NULL,
    title       TEXT NOT NULL,
    author      TEXT NOT NULL,
    state       TEXT NOT NULL,
    priority    TEXT NOT NULL,
    repo_id     TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    labels      TEXT NOT NULL DEFAULT '[]',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE(repo_id, number)
);

CREATE INDEX idx_issues_repo_id ON issues(repo_id);
CREATE INDEX idx_issues_state   ON issues(state);

-- ── activity ───────────────────────────────────────────────────────

CREATE TABLE activity (
    id               TEXT PRIMARY KEY,
    activity_type    TEXT NOT NULL,
    actor            TEXT NOT NULL,
    repo_id          TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    pull_request_id  TEXT REFERENCES pull_requests(id) ON DELETE SET NULL,
    issue_id         TEXT REFERENCES issues(id) ON DELETE SET NULL,
    message          TEXT NOT NULL,
    is_read          INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL
);

CREATE INDEX idx_activity_repo_id          ON activity(repo_id);
CREATE INDEX idx_activity_created_at       ON activity(created_at);
CREATE INDEX idx_activity_is_read          ON activity(is_read);
CREATE INDEX idx_activity_pull_request_id  ON activity(pull_request_id);
CREATE INDEX idx_activity_issue_id         ON activity(issue_id);

-- ── workspaces ─────────────────────────────────────────────────────

CREATE TABLE workspaces (
    id                   TEXT PRIMARY KEY,
    repo_id              TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    pull_request_number  INTEGER NOT NULL,
    state                TEXT NOT NULL DEFAULT 'active',
    worktree_path        TEXT,
    session_id           TEXT,
    last_active_at       TEXT,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    UNIQUE(repo_id, pull_request_number)
);

CREATE INDEX idx_workspaces_repo_id ON workspaces(repo_id);
CREATE INDEX idx_workspaces_state   ON workspaces(state);

-- ── workspace_notes ────────────────────────────────────────────────

CREATE TABLE workspace_notes (
    id            TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    content       TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE INDEX idx_workspace_notes_workspace_id ON workspace_notes(workspace_id);

-- ── config (key-value) ─────────────────────────────────────────────

CREATE TABLE config (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

-- ── notification_log ───────────────────────────────────────────────

CREATE TABLE notification_log (
    id          TEXT PRIMARY KEY,
    event_type  TEXT NOT NULL,
    event_id    TEXT NOT NULL,
    notified_at TEXT NOT NULL,
    UNIQUE(event_type, event_id)
);

-- ── Default configuration ──────────────────────────────────────────

INSERT INTO config (key, value) VALUES ('poll_interval_secs', '300');
INSERT INTO config (key, value) VALUES ('max_active_workspaces', '3');
