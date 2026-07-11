-- Purpose: Create the local-first core schema for Node Tide MVP.
-- Risk: Additive initial migration only. No user data exists before this migration.
-- Rollback: For pre-release development, delete the local database after exporting test data.

CREATE TABLE IF NOT EXISTS knowledge_objects (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    title TEXT,
    canonical_url TEXT,
    source_platform TEXT,
    author TEXT,
    privacy_level TEXT NOT NULL CHECK (
        privacy_level IN ('public', 'personal', 'sensitive', 'secret')
    ),
    lifecycle_status TEXT NOT NULL CHECK (
        lifecycle_status IN (
            'captured', 'parsed', 'enriched', 'evaluated',
            'triaged', 'archived', 'deleted', 'failed'
        )
    ),
    failure_reason TEXT,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_user_status
    ON knowledge_objects(user_id, lifecycle_status, updated_at);

CREATE INDEX IF NOT EXISTS idx_knowledge_objects_type
    ON knowledge_objects(object_type);

CREATE TABLE IF NOT EXISTS source_snapshots (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    snapshot_type TEXT NOT NULL,
    storage_uri TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    parser_id TEXT,
    parser_version TEXT,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_source_snapshots_object
    ON source_snapshots(object_id, captured_at);

CREATE TABLE IF NOT EXISTS parsed_documents (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    source_snapshot_id TEXT,
    title TEXT,
    text_content TEXT NOT NULL,
    markdown_content TEXT,
    language TEXT,
    word_count INTEGER,
    content_hash TEXT NOT NULL,
    parser_id TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (source_snapshot_id) REFERENCES source_snapshots(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_parsed_documents_object_hash
    ON parsed_documents(object_id, content_hash);

CREATE INDEX IF NOT EXISTS idx_parsed_documents_object
    ON parsed_documents(object_id, created_at);

CREATE TABLE IF NOT EXISTS ai_analysis (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    parsed_document_id TEXT,
    analysis_type TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    summary TEXT,
    category TEXT,
    tags_json TEXT,
    key_points_json TEXT,
    claims_json TEXT,
    action_items_json TEXT,
    risks_json TEXT,
    quality_score REAL,
    confidence REAL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (parsed_document_id) REFERENCES parsed_documents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_analysis_object_created
    ON ai_analysis(object_id, created_at);

CREATE TABLE IF NOT EXISTS ai_traces (
    id TEXT PRIMARY KEY,
    analysis_id TEXT,
    object_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    capability TEXT NOT NULL CHECK (
        capability IN ('chat', 'embedding', 'rerank', 'vision')
    ),
    prompt_template_id TEXT,
    prompt_template_version TEXT,
    input_snapshot_id TEXT,
    input_parsed_document_id TEXT,
    input_hash TEXT,
    output_hash TEXT,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    estimated_cost_usd REAL,
    latency_ms INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (analysis_id) REFERENCES ai_analysis(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (input_snapshot_id) REFERENCES source_snapshots(id) ON DELETE SET NULL,
    FOREIGN KEY (input_parsed_document_id) REFERENCES parsed_documents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_traces_object
    ON ai_traces(object_id, created_at);

CREATE TABLE IF NOT EXISTS evaluation_runs (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    evaluator_type TEXT NOT NULL,
    evaluator_version TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('planned', 'running', 'passed', 'failed', 'skipped', 'blocked')
    ),
    plan_json TEXT,
    input_json TEXT,
    output_json TEXT,
    dimensions_json TEXT,
    evidence_json TEXT,
    limitations_json TEXT,
    next_actions_json TEXT,
    score REAL,
    verdict TEXT NOT NULL CHECK (
        verdict IN ('high_value', 'useful', 'situational', 'low_value', 'unsafe', 'unknown')
    ),
    failure_reason TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evaluation_runs_object_created
    ON evaluation_runs(object_id, created_at);

CREATE TABLE IF NOT EXISTS evaluation_artifacts (
    id TEXT PRIMARY KEY,
    evaluation_run_id TEXT NOT NULL,
    artifact_type TEXT NOT NULL,
    storage_uri TEXT NOT NULL,
    content_hash TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (evaluation_run_id) REFERENCES evaluation_runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('user', 'ai_generated', 'imported'))
);

CREATE TABLE IF NOT EXISTS object_tags (
    object_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (object_id, tag_id),
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    collection_type TEXT NOT NULL DEFAULT 'manual',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS collection_objects (
    collection_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (collection_id, object_id),
    FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
    object_id UNINDEXED,
    parsed_document_id UNINDEXED,
    title,
    author,
    content,
    ai_summary,
    tokenize='unicode61'
);

CREATE TABLE IF NOT EXISTS vector_chunks_meta (
    chunk_id INTEGER PRIMARY KEY,
    object_id TEXT NOT NULL,
    parsed_document_id TEXT,
    chunk_index INTEGER NOT NULL,
    chunk_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    embedding_provider TEXT,
    embedding_model TEXT,
    embedding_dimensions INTEGER NOT NULL DEFAULT 1536,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (parsed_document_id) REFERENCES parsed_documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vector_chunks_meta_object
    ON vector_chunks_meta(object_id, chunk_index);

CREATE TABLE IF NOT EXISTS object_relations (
    id TEXT PRIMARY KEY,
    from_object_id TEXT NOT NULL,
    to_object_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (from_object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE,
    FOREIGN KEY (to_object_id) REFERENCES knowledge_objects(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_object_relations_unique
    ON object_relations(from_object_id, to_object_id, relation_type, source);

CREATE TABLE IF NOT EXISTS background_jobs (
    id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'blocked')
    ),
    object_id TEXT,
    payload_json TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    next_run_at TEXT,
    locked_at TEXT,
    locked_by TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (object_id) REFERENCES knowledge_objects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_background_jobs_status_next
    ON background_jobs(status, next_run_at);

CREATE INDEX IF NOT EXISTS idx_background_jobs_object
    ON background_jobs(object_id, created_at);

CREATE TABLE IF NOT EXISTS domain_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_version INTEGER NOT NULL DEFAULT 1,
    user_id TEXT NOT NULL,
    object_id TEXT,
    causation_id TEXT,
    correlation_id TEXT,
    payload_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_domain_events_unprocessed
    ON domain_events(processed_at, occurred_at);

CREATE INDEX IF NOT EXISTS idx_domain_events_object
    ON domain_events(object_id, occurred_at);

CREATE TABLE IF NOT EXISTS plugin_manifests (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    plugin_kind TEXT NOT NULL CHECK (
        plugin_kind IN ('connector', 'parser', 'evaluator', 'model_provider', 'sync_provider', 'exporter')
    ),
    manifest_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    installed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS plugin_permissions (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    permission_kind TEXT NOT NULL,
    scope TEXT,
    required INTEGER NOT NULL DEFAULT 0,
    granted INTEGER NOT NULL DEFAULT 0,
    granted_at TEXT,
    FOREIGN KEY (plugin_id) REFERENCES plugin_manifests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_plugin_permissions_plugin
    ON plugin_permissions(plugin_id);

CREATE TABLE IF NOT EXISTS model_provider_configs (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    chat_base_url TEXT,
    embeddings_base_url TEXT,
    default_chat_model TEXT,
    default_embedding_model TEXT,
    capabilities_json TEXT NOT NULL,
    secret_ref TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS deletion_tombstones (
    id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    deletion_mode TEXT NOT NULL CHECK (
        deletion_mode IN ('soft_delete', 'purge', 'export_then_delete')
    ),
    purge_status TEXT NOT NULL CHECK (
        purge_status IN ('pending', 'running', 'completed', 'failed')
    ),
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_deletion_tombstones_status
    ON deletion_tombstones(purge_status, created_at);

CREATE TABLE IF NOT EXISTS local_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT,
    action TEXT NOT NULL,
    object_id TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created
    ON audit_logs(user_id, created_at);
