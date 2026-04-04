-- Add head_ref_name (branch name) to pull_requests for workspace creation.
ALTER TABLE pull_requests ADD COLUMN head_ref_name TEXT NOT NULL DEFAULT '';
