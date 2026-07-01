-- Purpose: persist a privacy-bounded execution trace for every evaluation run.
-- Risk: additive table and indexes only; existing runs remain valid without a trace.
-- Rollback: retain trace rows for auditability; older binaries ignore this table.

CREATE TABLE IF NOT EXISTS evaluation_traces (
    id TEXT PRIMARY KEY,
    schema_version INTEGER NOT NULL DEFAULT 1,
    evaluation_run_id TEXT NOT NULL UNIQUE,
    request_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    evaluator_type TEXT NOT NULL,
    evaluator_version TEXT NOT NULL,
    execution_kind TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    output_hash TEXT,
    timeout_ms INTEGER NOT NULL CHECK (timeout_ms > 0),
    latency_ms INTEGER,
    status TEXT NOT NULL CHECK (status IN ('planned', 'running', 'passed', 'failed')),
    error_code TEXT,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (evaluation_run_id) REFERENCES evaluation_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evaluation_traces_correlation
    ON evaluation_traces(correlation_id);

CREATE INDEX IF NOT EXISTS idx_evaluation_traces_status_created
    ON evaluation_traces(status, created_at);
