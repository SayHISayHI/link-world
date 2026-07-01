-- Purpose: preserve immutable Evaluation history while linking explicit retries.
-- Risk: additive nullable self-reference and index only; existing runs remain roots.
-- Rollback: retain lineage metadata; older binaries ignore the nullable column.

ALTER TABLE evaluation_runs
    ADD COLUMN retry_of_run_id TEXT REFERENCES evaluation_runs(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_evaluation_runs_retry_parent
    ON evaluation_runs(retry_of_run_id, created_at)
    WHERE retry_of_run_id IS NOT NULL;
