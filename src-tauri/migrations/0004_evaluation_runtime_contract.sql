-- Purpose: add durable identity and schema-version metadata for idempotent evaluation runs.
-- Risk: additive columns and indexes only; existing evaluation rows remain readable.
-- Rollback: retain nullable identity columns; consumers must continue accepting legacy NULL values.

ALTER TABLE evaluation_runs ADD COLUMN request_id TEXT;
ALTER TABLE evaluation_runs ADD COLUMN correlation_id TEXT;
ALTER TABLE evaluation_runs ADD COLUMN plan_schema_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE evaluation_runs ADD COLUMN input_schema_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE evaluation_runs ADD COLUMN output_schema_version INTEGER NOT NULL DEFAULT 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_evaluation_runs_request_id
    ON evaluation_runs(request_id)
    WHERE request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_evaluation_runs_status_created
    ON evaluation_runs(status, created_at);
