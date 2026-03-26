-- Add diff statistics to pull_requests for priority scoring.
ALTER TABLE pull_requests ADD COLUMN additions INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pull_requests ADD COLUMN deletions INTEGER NOT NULL DEFAULT 0;
